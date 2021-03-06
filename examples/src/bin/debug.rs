// Copyright (c) 2016 The vulkano developers
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>,
// at your option. All files in the project carrying such
// notice may not be copied, modified, or distributed except
// according to those terms.

extern crate vulkano;

use std::ffi::CString;

use vulkano::device::{Device, DeviceExtensions};
use vulkano::format::Format;
use vulkano::image::ImmutableImage;
use vulkano::image::Dimensions;
use vulkano::instance;
use vulkano::instance::{Instance, InstanceExtensions, PhysicalDevice};
use vulkano::instance::debug::{DebugCallback, MessageType, MessageSeverity,MessageFilter};

use vulkano::image::StorageImage;
use vulkano::command_buffer::CommandBuffer;
use vulkano::command_buffer::AutoCommandBufferBuilder;

use vulkano::command_buffer::submit::SubmitCommandBufferBuilder;

use vulkano::sync::Fence;
use vulkano::buffer::BufferAccess;

use vulkano::buffer::CpuAccessibleBuffer;
use vulkano::buffer::BufferUsage;
use vulkano::image::ImageAccess;

fn main() {
    // Vulkano Debugging Example Code
    //
    // This example code will demonstrate using the debug functions of the Vulkano API.
    //
    // Vulkan has a concept of "Validation Layers". These layers are extra debug tools that will validate
    // and inform you about incorrect usage of the API. Turning these on while developing is strongly advised. 
    // The documentation about layers can be found here: 
    // https://www.khronos.org/registry/vulkan/specs/1.1-extensions/html/vkspec.html#extended-functionality-layers

    // Vulkan also has a debug utility extension which you can enable to extend the amount of information to receive when a validation layer
    // reports a message. More info can be found here:
    // https://www.khronos.org/registry/vulkan/specs/1.1-extensions/html/vkspec.html#VK_EXT_debug_utils
    //
    // .. but if you just want a template of code that has everything ready to go then follow
    // this example. First, enable debugging using this extension: VK_EXT_debug_report
    let extensions = InstanceExtensions {
        ext_debug_utils: true,
        ..InstanceExtensions::none()
    };

    // You also need to specify which debugging layers
    // your code should use. Each layer is a bunch of checks or messages that provide information of
    // some sort.
    //
    // The main layer you might want is: VK_LAYER_LUNARG_standard_validation
    // This includes a number of the other layers for you and is quite detailed.
    //
    // Additional layers can be installed (gpu vendor provided, something you found on GitHub, etc)
    // and you should verify that list for safety - Vulkano will return an error if you specify
    // any layers that are not installed on this system. That code to do could look like this:
    println!("List of Vulkan debugging layers available to use:");
    let mut layers = instance::layers_list().unwrap();
    while let Some(l) = layers.next() {
        println!("\t{}", l.name());
    }

    // NOTE: To simplify the example code we won't verify these layer(s) are actually in the layers list:
    let layer = "VK_LAYER_LUNARG_standard_validation";
    let layers = vec![layer];

    // Important: pass the extension(s) and layer(s) when creating the vulkano instance
    let instance = Instance::new(None, &extensions, layers).expect("failed to create Vulkan instance");

    ///////////////////////////////////////////////////////////////////////////////////////////////////////////////
    // After creating the instance we must register the debugging callback.                                      //
    ///////////////////////////////////////////////////////////////////////////////////////////////////////////////

    // Note: If you let this debug_callback binding fall out of scope then the callback will stop providing events
    // Note: There is a helper method too: DebugCallback::errors_and_warnings(&instance, |msg| {...

    // Creates a message filter that shows all messages.
    // You can either use the built in functions or construct your filter manually
    let all = MessageFilter{
        types: MessageType::all(),
        severity: MessageSeverity::ERROR | MessageSeverity::WARNING,
    };

    // Initializes a DebugUtilsMessenger and binds the callback to our callback.
    // We use our message filter to receive all messages.
    let debug_callback = DebugCallback::new(&instance, all, |msg| {
        let ty = if msg.severity & MessageSeverity::ERROR != MessageSeverity::none(){
            "error"
        } else if msg.severity & MessageSeverity::WARNING != MessageSeverity::none() {
            "warning"
        } else if msg.severity & MessageSeverity::WARNING != MessageSeverity::none() && msg.ty & MessageType::PERFORMANCE != MessageType::none() {
            "performance_warning"
        } else if msg.severity & MessageSeverity::INFO != MessageSeverity::none() {
            "information"
        } else if msg.severity & MessageSeverity::VERBOSE != MessageSeverity::none() {
            "debug"
        } else {
            panic!("no-impl");
        };

        // An example of how to access the incoming labels of the queue and command buffer.
        // note: Currently you can not insert a tag into a queue yet as this isn't exposed yet.
        println!("{}: {}", ty, msg.description);
        println!("Queue Labels({}):", msg.queue_labels.len());
        for obj in msg.queue_labels.iter() {
            println!(" QueueLabel: {}, {:?}",obj.name, obj.color);
        }

        // These are all the labels inserted in our command buffer when an error is reported related to that command buffer.
        println!("Command Labels({}):", msg.command_buffer_labels.len());
        for obj in msg.command_buffer_labels.iter() {
            println!(" CmdBufLabel: {}, {:?}",obj.name, obj.color);
        }

        // Objects are all the relevant vulkan objects that this message is relevant to.
        println!("Objects({}):", msg.objects.len());
        for obj in msg.objects.iter() {
            println!("  Obj: {}, 0x{:x}, {}",obj.name, obj.handle, obj.ty);
        }

    }).unwrap();
    use vulkano::instance::debug::Message;

    fn submit_error(callback : &vulkano::instance::debug::DebugCallback,name: &str, desc : &str, obj : &str) {
        callback.submit_debug_message(Message{
            ty: MessageType::GENERAL,
            severity: MessageSeverity::ERROR,
            id_number: 0,
            id_name: name,
            description: desc,
            objects: vec![
                vulkano::instance::debug::ObjectNameInfo{
                    ty: 100,
                    handle: 0,
                    name: obj
                }
            ],
            ..Message::default()
        });
    }

    fn submit_warning(callback : &vulkano::instance::debug::DebugCallback,name: &str, desc : &str, obj : &str) {
        callback.submit_debug_message(Message{
            ty: MessageType::GENERAL,
            severity: MessageSeverity::WARNING,
            id_number: 0,
            id_name: name,
            description: desc,
            objects: vec![
                vulkano::instance::debug::ObjectNameInfo{
                    ty: 100,
                    handle: 0,
                    name: obj
                }
            ],
            ..Message::default()
        });
    }

    submit_error(&debug_callback, "DebugMsg", "This is a debug error message!", "Dummy object");
    submit_warning(&debug_callback, "DebugMsg", "This is a debug warning message!", "Dummy object");


    ///////////////////////////////////////////////////////////////////////////////////////////////////////////////
    // Create Vulkan objects in the same way as the other examples                                               //
    ///////////////////////////////////////////////////////////////////////////////////////////////////////////////

    let physical = PhysicalDevice::enumerate(&instance).next().expect("no device available");
    let queue_family = physical.queue_families().next().expect("couldn't find a queue family");
    let (device, mut queues) = Device::new(physical, physical.supported_features(), &DeviceExtensions::none(), vec![(queue_family, 0.5)]).expect("failed to create device");
    let queue = queues.next().unwrap();

    // Create an image in order to generate some additional logging:
    let pixel_format = Format::R8G8B8A8Uint;
    let dimensions = Dimensions::Dim2d { width: 4096, height: 4096 };
    const DATA: [[u8; 4]; 4096*4096] = [[0; 4]; 4096 * 4096];
    ImmutableImage::from_iter(DATA.iter().cloned(), dimensions, pixel_format,
                                               queue.clone()).unwrap();

    let buffer1 = CpuAccessibleBuffer::from_iter(device.clone(), BufferUsage::all(), (0..512*512*4).map(|_| 0)).unwrap();
    let img = StorageImage::new(device.clone(), vulkano::image::Dimensions::Dim2d{width:512,height:512}, Format::R8G8B8A8Uint, Some(queue.family())).unwrap();

    // VK_EXT_debug_utils allows setting object tags which will be reported whenever a message that is relevant to these objects
    // is being reported. See the debug callback for information on how to access them.
    device.set_object_name(img.inner().image, &CString::new("DestinationImage").unwrap()).ok();
    device.set_object_name(buffer1.inner().buffer, &CString::new("DestinationImage").unwrap()).ok();

    // Instead of using something like an AutoCommandBufferBuilder this is using a UnsafeCommandBufferBuilder so we can
    // forcefully insert incorrect function calls and trigger validation errors and a crash. In a real application you should probably
    // use the auto command buffer builder. 
    let fence = Fence::from_pool(device.clone()).unwrap();

    let mut builder = AutoCommandBufferBuilder::new(device.clone(), queue_family).unwrap();
    builder = builder.begin_debug_label("Begin Of Buffer", [0.9,0.7,1.0,1.0]);
    builder = builder.insert_debug_label("CopyStarting", [1.0,1.0,1.0,1.0]);
    builder = builder.copy_buffer_to_image(buffer1.clone(), img.clone()).unwrap();
    builder = builder.insert_debug_label("CopyDone", [1.0,1.0,1.0,1.0]);
    builder = builder.end_debug_label();


    let cmd_buffer = builder.build().unwrap();

    // Tags can be set on all kinds of objects.
    
    unsafe {
        device.set_object_name(cmd_buffer.inner(), &CString::new("BadCommandBuffer").unwrap()).ok();

        let mut builder = SubmitCommandBufferBuilder::new();
        builder.add_command_buffer(cmd_buffer.inner());
        builder.set_fence_signal(&fence);
        builder.submit(&queue).ok();

        fence.wait(None).ok();
    }
    // (At this point you should see a bunch of messages printed to the terminal window - have fun debugging!)
}
