use winit::{
    event::{
        Event,
        WindowEvent,
    },
    event_loop::ControlFlow,
};
use bui::{
    rect,
    renderer,
    resolution_buffer::ResolutionBuffer,
    ellipse::*,
};
use std::{
    thread,
    time::{
        Duration,
        Instant,
    },
};
use rodio::Source;
use spectrum_analyzer::{
    samples_fft_to_spectrum,
    FrequencyLimit,
    FrequencySpectrum,
};

use sound_galaxy::*;

const PARTICLE_COUNT: usize = 2048;
const SAMPLE_COUNT: usize = PARTICLE_COUNT*2;

type EllipseRendererRef<'a> = &'a mut EllipseRenderer;
type RendererRef<'a> = &'a renderer::Renderer;
type ResolutionBufferRef<'a> = &'a mut ResolutionBuffer;
type ParticleVec = Vec<Particle>;
constrainer::create_constrainer!(Constrainer {
    dynamic resx f32
    dynamic resy f32
    dynamic particles ParticleVec
    external ellipse_renderer EllipseRendererRef
    external renderer RendererRef
    external resolution_buffer ResolutionBufferRef

    listener set_resolution_buffer (resx, resy, resolution_buffer, renderer) {
        resolution_buffer.set(&[resx, resy], renderer.queue());
    }

    listener update_ellipse_buffer (resx, resy, &particles, ellipse_renderer, renderer) {
        // println!("Updating ellipse buffer!");
        let mut ellipse_buffer: Vec<EllipseBuffer> = Vec::with_capacity(PARTICLE_COUNT);
        for particle in particles {
            ellipse_buffer.push(EllipseDescriptor {
                sizing: rect::FillAspect {
                    placement_area: rect::SizeAndCenter {
                        sx: particle.diameter,
                        sy: particle.diameter,
                        cx: particle.x,
                        cy: particle.y,
                    },
                    centerx: 0.0,
                    centery: 0.0,
                    resx: resx,
                    resy: resy,
                    aspect: 1.0,
                }.into(),
                r: particle.r,
                g: particle.g,
                b: particle.b,
                a: 1.0,
            }.into())
        }
    
        ellipse_renderer.set_ellipse_buffer(renderer.queue(), ellipse_buffer.as_slice());
    }

    opgenset (resx, resy)
});

// This function should be able to be generated automatically. Add feature to Constrainer for it.
impl Constrainer {
    pub fn set_particles_with_spectrum_and_deltatime(&mut self, spectrum: FrequencySpectrum, deltatime: f32, ellipse_renderer: EllipseRendererRef, renderer: RendererRef) {
        let spectrum_data = spectrum.data();
        for (i, particle) in &mut self.particles.iter_mut().enumerate() {
            let mut diameter = spectrum_data[i].1.val()*0.0001;
            if diameter > 0.02 {
                diameter = 0.02+(diameter-0.02)/50.0;
            }
            if diameter < 0.001 {
                diameter = diameter*10.0;
                if diameter > 0.001 {
                    diameter = 0.001;
                }
            }
            particle.diameter = diameter;

            particle.y -= diameter*deltatime*20.0;
            while particle.y < -1.0 {
                particle.y += 2.0;
            }
        }
        Self::update_ellipse_buffer(self.resx, self.resy, &self.particles, ellipse_renderer, renderer);
    }
}

fn main() {
    let audio_file_name = if let Some(name) = std::env::args().nth(1) {
        name
    } else {
        println!("Defaulting to ./input.mp3");
        "input.mp3".to_string()
    };
    let audio_file = std::fs::File::open(audio_file_name).expect("Failed to open audio file");

    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_title("Sound Galaxy")
        .build(&event_loop).unwrap();
    let mut renderer = futures::executor::block_on(renderer::Renderer::new(&window));
    let mut resolution_buffer = ResolutionBuffer::new(renderer.device());
    let mut ellipse_renderer = EllipseRenderer::new(renderer.device(), renderer.config().format, &resolution_buffer, PARTICLE_COUNT as wgpu::BufferAddress);

    let (_stream, stream_handle) = rodio::OutputStream::try_default().unwrap();
    let sink = rodio::Sink::try_new(&stream_handle).unwrap();
    let source = rodio::Decoder::new(audio_file).unwrap().delay(std::time::Duration::from_secs(1));
    let source = source.buffered();
    let samples: Vec<f32> = source.clone().map(|input| -> _ {
        input as f32
    }).collect();
    println!("samples count: {}", samples.len());
    let sample_rate = source.sample_rate();
    println!("sample_rate: {}", sample_rate);
    println!("channels: {}", source.channels());
    sink.pause();
    sink.append(source);
    
    let mut particles = Vec::with_capacity(PARTICLE_COUNT);
    for i in 0..PARTICLE_COUNT {
        let x = if i < PARTICLE_COUNT/2 {
            i as f32/(PARTICLE_COUNT as f32-1.0)*-2.0
        } else {
            (i as f32/(PARTICLE_COUNT as f32-1.0))*-2.0+2.0
        };
        particles.push(Particle::new(x));
    }

    let mut constrainer = Constrainer::new(window.inner_size().width as f32, window.inner_size().height as f32, particles, &mut ellipse_renderer, &renderer, &mut resolution_buffer);
    
    let timer = Instant::now();
    let mut last_frame_time = Instant::now();
    let mut deltatime = 0.0;
    sink.play();
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit
            },
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => {
                match event {
                    WindowEvent::Resized(physical_size) => {
                        renderer.resize(*physical_size);
                        constrainer.set_resx_resy(
                            physical_size.width as f32,
                            physical_size.height as f32,
                            &mut ellipse_renderer,
                            &renderer,
                            &mut resolution_buffer,
                        )
                    },
                    WindowEvent::ScaleFactorChanged {
                        new_inner_size,
                        ..
                    } => {
                        renderer.resize(**new_inner_size);
                        constrainer.set_resx_resy(
                            new_inner_size.width as f32,
                            new_inner_size.height as f32,
                            &mut ellipse_renderer,
                            &renderer,
                            &mut resolution_buffer,
                        )
                    },
                    _ => {}
                }
            },
            Event::RedrawRequested(_) => {
                match renderer.surface().get_current_texture() {
                    Ok(surface_texture) => {
                        let view = surface_texture.texture.create_view(&wgpu::TextureViewDescriptor::default());
                        let mut encoder = renderer.device().create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("Render encoder"),
                        });
                        let current_sample = (timer.elapsed().as_secs_f32()*2.0*sample_rate as f32) as usize;
                        if current_sample >= samples.len() {
                            println!("Ran out of samples. Exiting...");
                            *control_flow = ControlFlow::Exit;
                        } else if current_sample > SAMPLE_COUNT {
                            let current_samples = &samples[(current_sample-SAMPLE_COUNT)..current_sample];
                            let spectrum = samples_fft_to_spectrum(
                                current_samples,
                                sample_rate,
                                FrequencyLimit::All,
                                Some(&spectrum_analyzer::scaling::divide_by_N),
                            ).unwrap();
                            constrainer.set_particles_with_spectrum_and_deltatime(spectrum, deltatime, &mut ellipse_renderer, &renderer);
                        }
                        ellipse_renderer.render_all(&mut encoder, &view, wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0
                        }));
                        renderer.queue().submit(std::iter::once(encoder.finish()));
                        surface_texture.present();
                    },
                    Err(wgpu::SurfaceError::Lost) => {
                        eprintln!("Surface lost!");
                        renderer.reconfigure();
                    },
                    Err(wgpu::SurfaceError::OutOfMemory) => {
                        eprintln!("Out of memory!");
                        *control_flow = ControlFlow::Exit;
                    },
                    Err(e) => {
                        eprintln!("Surface error: {:?}", e);
                    },
                };
                thread::sleep(Duration::from_secs_f32(1.0/150.0).saturating_sub(last_frame_time.elapsed())); // This limits FPS for my poor laptop that crashes if it runs at max fps
                // println!("{}", 1.0/last_frame_time.elapsed().as_secs_f32()); // prints FPS
                deltatime = last_frame_time.elapsed().as_secs_f32();
                last_frame_time = Instant::now();
                window.request_redraw();
            },
            _ => ()
        }
    });
}