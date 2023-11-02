use bevy::render::{
    render_resource::{
        encase::internal::WriteInto, Buffer, BufferDescriptor, BufferUsages,
        CommandEncoderDescriptor, ShaderType, UniformBuffer,
    },
    renderer::{RenderDevice, RenderQueue},
};

pub fn read_buffer_f32(buffer: &Buffer, device: &RenderDevice, queue: &RenderQueue) {
    let mut command_encoder =
        device.create_command_encoder(&CommandEncoderDescriptor { label: None });

    let readback_buffer = device.create_buffer(&BufferDescriptor {
        label: None,
        size: buffer.size(),
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    command_encoder.copy_buffer_to_buffer(buffer, 0, &readback_buffer, 0, buffer.size());
    queue.submit([command_encoder.finish()]);

    readback_buffer
        .clone()
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let err = result.err();
            if err.is_some() {
                panic!("{}", err.unwrap().to_string());
            }
            let contents = readback_buffer.slice(..).get_mapped_range();
            let readback = contents
                .chunks_exact(std::mem::size_of::<f32>())
                .map(|bytes| f32::from_ne_bytes(bytes.try_into().unwrap()))
                .collect::<Vec<_>>();
            println!("Output: {readback:?}");
        });
}

pub fn read_buffer_u32(buffer: &Buffer, device: &RenderDevice, queue: &RenderQueue) {
    let mut command_encoder =
        device.create_command_encoder(&CommandEncoderDescriptor { label: None });

    let readback_buffer = device.create_buffer(&BufferDescriptor {
        label: None,
        size: buffer.size(),
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    command_encoder.copy_buffer_to_buffer(buffer, 0, &readback_buffer, 0, buffer.size());
    queue.submit([command_encoder.finish()]);

    readback_buffer
        .clone()
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let err = result.err();
            if err.is_some() {
                panic!("{}", err.unwrap().to_string());
            }
            let contents = readback_buffer.slice(..).get_mapped_range();
            let readback = contents
                .chunks_exact(std::mem::size_of::<u32>())
                .map(|bytes| u32::from_ne_bytes(bytes.try_into().unwrap()))
                .collect::<Vec<_>>();
            println!("Output: {readback:?}");
        });
}

pub fn struct_to_buffer<T: ShaderType + WriteInto>(
    s: T,
    render_device: &RenderDevice,
    render_queue: &RenderQueue,
) -> UniformBuffer<T> {
    let mut buffer = UniformBuffer::from(s);
    buffer.set_label(Some("flow_field_globals"));
    buffer.write_buffer(render_device, render_queue);
    buffer
}
