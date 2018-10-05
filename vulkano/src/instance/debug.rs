// Copyright (c) 2016 The vulkano developers
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>,
// at your option. All files in the project carrying such
// notice may not be copied, modified, or distributed except
// according to those terms.

//! Debug callback called by intermediate layers or by the driver.
//!
//! When working on an application, it is recommended to register a debug callback. For example if
//! you enable the validation layers provided by the official Vulkan SDK, they will warn you about
//! invalid API usages or performance problems by calling this callback. The callback can also
//! be called by the driver or by whatever intermediate layer is activated.
//!
//! Note that the vulkano library can also emit messages to warn you about performance issues.
//! TODO: ^ that's not the case yet, need to choose whether we keep this idea
//!
//! # Example
//!
//! ```
//! # use vulkano::instance::Instance;
//! # use std::sync::Arc;
//! # let instance: Arc<Instance> = return;
//! use vulkano::instance::debug::DebugCallback;
//!
//! let _callback = DebugCallback::errors_and_warnings(&instance, |msg| {
//!     println!("Debug callback: {:?}", msg.description);
//! }).ok();
//! ```
//!
//! The type of `msg` in the callback is [`Message`](struct.Message.html).
//!
//! Note that you must keep the `_callback` object alive for as long as you want your callback to
//! be callable. If you don't store the return value of `DebugCallback`'s constructor in a
//! variable, it will be immediately destroyed and your callback will not work.
//!

use std::error;
use std::ffi::CStr;
use std::fmt;
use std::mem;
use std::os::raw::{c_char, c_void};
use std::panic;
use std::ptr;
use std::sync::Arc;
use std::cmp::PartialEq;

use std::ops::{BitOr,BitOrAssign, BitAnd, BitAndAssign, BitXor, BitXorAssign};

use instance::Instance;

use Error;
use VulkanObject;
use check_errors;
use vk;

/// Registration of a callback called by validation layers.
///
/// The callback can be called as long as this object is alive.
#[must_use = "The DebugCallback object must be kept alive for as long as you want your callback \
              to be called"]
pub struct DebugCallback {
    instance: Arc<Instance>,
    debug_utils_messenger: vk::DebugUtilsMessengerEXT,
    user_callback: Box<Box<Fn(&Message)>>,
}

impl DebugCallback {
    /// Initializes a debug callback.
    ///
    /// Panics generated by calling `user_callback` are ignored.
    pub fn new<F>(instance: &Arc<Instance>, filter : MessageFilter, user_callback: F)
                  -> Result<DebugCallback, DebugCallbackCreationError>
        where F: Fn(&Message) + 'static + Send + panic::RefUnwindSafe
    {
        if !instance.loaded_extensions().ext_debug_utils {
            return Err(DebugCallbackCreationError::MissingExtension);
        }

        // Note that we need to double-box the callback, because a `*const Fn()` is a fat pointer
        // that can't be cast to a `*const c_void`.
        let user_callback = Box::new(Box::new(user_callback) as Box<_>);

        extern "system" fn callback(message_severity: vk::DebugUtilsMessageSeverityFlagBitsEXT, ty : vk::DebugUtilsMessageTypeFlagsEXT,
                                    callback_data : *const vk::DebugUtilsMessengerCallbackDataEXT, user_data : *mut c_void)
                                    -> u32 {
            unsafe {
                let user_callback = user_data as *mut Box<Fn()> as *const _;
                let user_callback: &Box<Fn(&Message)> = &*user_callback;

                let message_id_name = CStr::from_ptr((*callback_data).pMessageIdName)
                    .to_str()
                    .expect("debug callback message not utf-8");

                let message_id_number = (*callback_data).messageIdNumber;

                let description = CStr::from_ptr((*callback_data).pMessage)
                    .to_str()
                    .expect("debug callback message not utf-8");

                let command_buffer_labels : Vec<MessageLabel> = std::slice::from_raw_parts((*callback_data).pCmdBufLabels, (*callback_data).cmdBufLabelCount as usize)
                        .iter().map(|e| {
                            MessageLabel::from_raw(e)
                        }).collect();

                let queue_labels = std::slice::from_raw_parts((*callback_data).pQueueLabels, (*callback_data).queueLabelCount as usize)
                    .iter().map(|e|{
                        MessageLabel::from_raw(e)
                    }).collect();

                let objects = std::slice::from_raw_parts((*callback_data).pObjects, (*callback_data).objectCount as usize)
                    .iter().map(|e|{ 
                        ObjectNameInfo::from_raw(e)
                    }).collect();


                let flags = vk::DEBUG_UTILS_MESSAGE_TYPE_GENERAL_BIT_EXT | vk::DEBUG_UTILS_MESSAGE_TYPE_VALIDATION_BIT_EXT;
                let message = Message {
                    severity: MessageSeverity(message_severity),
                    ty: MessageType(ty),
                    id_number: message_id_number,
                    id_name: &message_id_name,
                    description: &description,
                    queue_labels: queue_labels,
                    command_buffer_labels: command_buffer_labels,
                    objects: objects,
                };

                // Since we box the closure, the type system doesn't detect that the `UnwindSafe`
                // bound is enforced. Therefore we enforce it manually.
                let _ = panic::catch_unwind(panic::AssertUnwindSafe(move || {
                                                                        user_callback(&message);
                                                                    }));

                vk::FALSE
            }
        }

        let infos = vk::DebugUtilsMessengerCreateInfoEXT {
            sType: vk::STRUCTURE_TYPE_DEBUG_UTILS_MESSENGER_CREATE_INFO_EXT,
            pNext: ptr::null(),
            flags: 0, // Placeholder
            messageSeverity: filter.severity.0,
            messageType: filter.types.0,
            pfnUserCallback: callback,
            pUserData: &*user_callback as &Box<_> as *const Box<_> as *const c_void as *mut _,
        };

        let vk = instance.pointers();

        let debug_utils_messenger = unsafe {
            let mut output = mem::uninitialized();
            check_errors(vk.CreateDebugUtilsMessengerEXT(instance.internal_object(),
                                                         &infos,
                                                         ptr::null(),
                                                         &mut output))?;
            output
        };

        Ok(DebugCallback {
               instance: instance.clone(),
               debug_utils_messenger: debug_utils_messenger,
               user_callback: user_callback,
           })
    }

    /// Initializes a debug callback with errors and warnings.
    ///
    /// Shortcut for `new(instance, MessageTypes::errors_and_warnings(), user_callback)`.
    #[inline]
    pub fn errors_and_warnings<F>(instance: &Arc<Instance>, user_callback: F)
                                  -> Result<DebugCallback, DebugCallbackCreationError>
        where F: Fn(&Message) + Send + 'static + panic::RefUnwindSafe
    {
        DebugCallback::new(instance, MessageFilter::errors_and_warnings(), user_callback)
    }
}

impl Drop for DebugCallback {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            let vk = self.instance.pointers();
            vk.DestroyDebugUtilsMessengerEXT(self.instance.internal_object(),
                                             self.debug_utils_messenger,
                                             ptr::null());
        }
    }
}

/// Type safe wrapper around `DebugUtilsMessageSeverityFlagBitsEXT`.
#[derive(Default,Clone, Copy)]
pub struct MessageType(u32);
impl MessageType{
    pub const GENERAL       :MessageType = MessageType(0x00000001);
    pub const VALIDATION    :MessageType = MessageType(0x00000002);
    pub const PERFORMANCE   :MessageType = MessageType(0x00000004);

    #[inline]
    pub fn all() -> MessageType{
        Self::GENERAL | Self::VALIDATION | Self::PERFORMANCE
    }

    #[inline]
    pub fn none() -> MessageType {
        MessageType(0)
    }
}


impl BitOr for MessageType {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        MessageType(self.0 | rhs.0)
    }
}

impl BitOrAssign for MessageType {
    fn bitor_assign(&mut self, rhs : Self){
        self.0 = self.0 | rhs.0;
    }
}

impl BitAnd for MessageType{
    type Output= Self;
    fn bitand(self, rhs: Self) -> Self{
        MessageType(self.0 & rhs.0)
    }
}

impl BitAndAssign for MessageType{
    fn bitand_assign(&mut self, rhs: Self){
        self.0 = self.0 & self.0;
    }
}

impl BitXor for MessageType {
    type Output = Self;
    fn bitxor(self, rhs: Self) -> Self{
        MessageType(self.0 ^ rhs.0)
    }
}

impl BitXorAssign for MessageType {
    fn bitxor_assign(&mut self, rhs: Self){
        self.0 = self.0 ^ rhs.0;
    }
}

/// Type safe wrapper around `DebugUtilsMessageTypeFlagBitsEXT`
#[derive(Default, Clone, Copy)]
pub struct MessageSeverity(u32);
impl MessageSeverity{
    pub const VERBOSE: MessageSeverity = MessageSeverity(0x00000001);
    pub const INFO   : MessageSeverity = MessageSeverity(0x00000010);
    pub const WARNING: MessageSeverity = MessageSeverity(0x00000100);
    pub const ERROR  : MessageSeverity = MessageSeverity(0x00001000);

    pub fn all() -> MessageSeverity{
        Self::VERBOSE | Self::INFO | Self::WARNING | Self::ERROR
    }
    pub fn none() -> MessageSeverity{
        MessageSeverity(0)
    }
}


impl BitOr for MessageSeverity {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        MessageSeverity(
            self.0 | rhs.0
        )
    }
}
impl BitOrAssign for MessageSeverity{
    fn bitor_assign(&mut self, rhs: Self ) {
        self.0 = self.0 | rhs.0;
    }
}

impl BitAndAssign for MessageSeverity{
    fn bitand_assign(&mut self, rhs: Self ) {
        self.0 = self.0 & rhs.0;
    }
}

impl BitXorAssign for MessageSeverity{
    fn bitxor_assign(&mut self, rhs: Self){
        self.0 = self.0 ^ rhs.0;
    }
}

impl BitXor for MessageSeverity {
    type Output = Self;
    fn bitxor(self, rhs: Self) -> Self{
        MessageSeverity(self.0 ^ rhs.0)
    }
}

impl BitAnd for MessageSeverity{
    type Output= Self;
    fn bitand(self, rhs: Self) -> Self::Output {
        MessageSeverity(self.0 & rhs.0)
    }
}

impl PartialEq for MessageSeverity {
    fn eq(&self, other : &MessageSeverity) -> bool {
        self.0 == other.0
    }
}

impl PartialEq for MessageType {
    fn eq(&self, other : &MessageType) -> bool {
        self.0 == other.0
    }
}

/// A message received by the callback.
pub struct Message<'a> {
    /// Type of message.
    pub ty: MessageType,

    /// Message severity
    pub severity: MessageSeverity,

    /// The message id number
    pub id_number : i32,

    /// The message ID in string form
    pub id_name : &'a str,

    /// Description of the message.
    pub description: &'a str,

    /// A list of labels belonging to the queue
    pub queue_labels : Vec<MessageLabel<'a>>,

    /// A list of labels belonging to the command buffer 
    pub command_buffer_labels : Vec<MessageLabel<'a>>,

    /// The objects this message refers to
    pub objects : Vec<ObjectNameInfo<'a>>
}

/// A message label that is a rust version of `DebugUtilsLabelEXT`
#[derive(Clone)]
pub struct MessageLabel<'a> {
    pub name : &'a str,

    pub color : [f32; 4],
}

impl<'a> MessageLabel<'a>{
    fn from_raw(utils_label : &vk::DebugUtilsLabelEXT) -> Self{
        let mut name = "";
        if utils_label.pLabelName != std::ptr::null(){
            name = unsafe { CStr::from_ptr(utils_label.pLabelName).to_str().unwrap() };
        }

        MessageLabel{
            name: &name,
            color: utils_label.color,
        }
    }
}

/// Data structure used by the `DebugCallback` to provide meaningfull info about objects
#[derive(Clone)]
pub struct ObjectNameInfo<'a> {
    pub ty : u32,
    pub handle : u64, 
    pub name : &'a str,
}

impl<'a> ObjectNameInfo<'a> {

    /// Constructs and `ObjectNameInfo` from the raw vulkan data structure `DebugUtilsObjectNameInfoEXT` 
    fn from_raw(utils_label : &vk::DebugUtilsObjectNameInfoEXT) -> Self{
        let mut name = "";
        if utils_label.pObjectName != std::ptr::null() {
            name = unsafe { 
                CStr::from_ptr(utils_label.pObjectName).to_str().unwrap() 
            };
        }
        ObjectNameInfo{
           ty: utils_label.objectType,
           handle: utils_label.objectHandle,
           name: name,
        }
    }


}

/// A filter that can be passed to `DebugCallback::new` to decide what messages
/// to passthrough to the callback.
#[derive(Clone)]
pub struct MessageFilter {
    pub types : MessageType, 
    pub severity : MessageSeverity,
}

impl MessageFilter {
    #[inline]
    pub fn all() -> MessageFilter {
        MessageFilter{
            severity: MessageSeverity::all(),
            types: MessageType::all(),
        }
    }

    #[inline]
    pub fn none() -> MessageFilter {
        MessageFilter{
            severity: MessageSeverity::none(),
            types: MessageType::none(),
        }
    }

    #[inline]
    pub fn errors_and_warnings() -> MessageFilter{
        MessageFilter{
            severity: MessageSeverity::WARNING | MessageSeverity::ERROR,
            types: MessageType::VALIDATION | MessageType::GENERAL
        }
    }
}

/// Error that can happen when creating a debug callback.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DebugCallbackCreationError {
    /// The `EXT_debug_report` extension was not enabled.
    MissingExtension,
}

impl error::Error for DebugCallbackCreationError {
    #[inline]
    fn description(&self) -> &str {
        match *self {
            DebugCallbackCreationError::MissingExtension =>
                "the `EXT_debug_utils` extension was not enabled",
        }
    }
}

impl fmt::Display for DebugCallbackCreationError {
    #[inline]
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(fmt, "{}", error::Error::description(self))
    }
}

impl From<Error> for DebugCallbackCreationError {
    #[inline]
    fn from(err: Error) -> DebugCallbackCreationError {
        panic!("unexpected error: {:?}", err)
    }
}