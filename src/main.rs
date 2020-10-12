/// Testing file for initializing servo with a window
/// - Uses glutin to create an OpenGL window
/// - Connects servo with the created OpenGL window
/// - Exposes the event loop to servo
/// - Exposes an OpenGL function pointer object
/// ---
use glutin::GlContext;
use log::debug;
use servo::compositing::windowing::EmbedderMethods;
use servo::embedder_traits::{EmbedderProxy, EventLoopWaker};
use servo::gl;
use std::rc::Rc;
use std::sync::Arc;

pub struct GlutinEventLoopWaker {
    pub proxy: Arc<glutin::EventsLoopProxy>,
}

impl EventLoopWaker for GlutinEventLoopWaker {
    // Use by servo to share the "event loop waker" across threads
    fn clone_box(&self) -> Box<dyn EventLoopWaker> {
        Box::new(GlutinEventLoopWaker {
            proxy: self.proxy.clone(),
        })
    }
    // Called by servo when the main thread needs to wake up
    fn wake(&self) {
        self.proxy.wakeup().expect("wakeup eventloop failed");
    }
}

// TODO: Finish integrating Servo from lib.rs
fn main() {
    println!("Servo version: {}", servo::config::servo_version());

    let mut event_loop = glutin::EventsLoop::new();

    let builder = glutin::WindowBuilder::new().with_dimensions(800, 600);
    let gl_version = glutin::GlRequest::Specific(glutin::Api::OpenGl, (3, 2));
    let context = glutin::ContextBuilder::new()
        .with_gl(gl_version)
        .with_vsync(true);
    let window = glutin::GlWindow::new(builder, context, &event_loop).unwrap();

    window.show();

    // let gl = unsafe {
    //     window
    //         .context()
    //         .make_current()
    //         .expect("Couldn't make window current");
    //     gl::GlFns::load_with(|s| window.context().get_proc_address(s) as *const _)
    // };

    // let event_loop_waker = Box::new(GlutinEventLoopWaker {
    //     proxy: Arc::new(event_loop.create_proxy()),
    // });

    // let window = Rc::new(Window {
    //     glutin_window: window,
    //     waker: event_loop_waker,
    //     gl: gl,
    // });

    // let embedder_callbacks = Box::new(ServoEmbedderCallbacks {
    //     xr_discovery: init_opts.xr_discovery,
    //     waker,
    //     gl: gl.clone(),
    // });

    // let mut servo = servo::Servo::new(embedder_callbacks, window.clone(), Ok("linux"));

    event_loop.run_forever(|_event| glutin::ControlFlow::Continue);
}

struct Window {
    // All these fields will be used in WindowMethods implementations
    glutin_window: glutin::GlWindow,
    waker: Box<dyn EventLoopWaker>,
    gl: Rc<dyn gl::Gl>,
}

struct ServoEmbedderCallbacks {
    waker: Box<dyn EventLoopWaker>,
    xr_discovery: Option<webxr::Discovery>,
    #[allow(unused)]
    gl: Rc<dyn gl::Gl>,
}

impl EmbedderMethods for ServoEmbedderCallbacks {
    #[cfg(feature = "uwp")]
    fn register_webxr(
        &mut self,
        registry: &mut webxr::MainThreadRegistry,
        embedder_proxy: EmbedderProxy,
    ) {
        use ipc_channel::ipc::{self, IpcReceiver};
        use webxr::openxr;
        debug!("EmbedderMethods::register_xr");
        assert!(
            self.xr_discovery.is_none(),
            "UWP builds should not be initialized with a WebXR Discovery object"
        );

        #[derive(Clone)]
        struct ContextMenuCallback(EmbedderProxy);

        struct ContextMenuFuture(IpcReceiver<ContextMenuResult>);

        impl openxr::ContextMenuProvider for ContextMenuCallback {
            fn open_context_menu(&self) -> Box<dyn openxr::ContextMenuFuture> {
                let (sender, receiver) = ipc::channel().unwrap();
                self.0.send((
                    None,
                    EmbedderMsg::ShowContextMenu(
                        sender,
                        Some("Would you like to exit the XR session?".into()),
                        vec!["Exit".into()],
                    ),
                ));

                Box::new(ContextMenuFuture(receiver))
            }
            fn clone_object(&self) -> Box<dyn openxr::ContextMenuProvider> {
                Box::new(self.clone())
            }
        }

        impl openxr::ContextMenuFuture for ContextMenuFuture {
            fn poll(&self) -> openxr::ContextMenuResult {
                if let Ok(result) = self.0.try_recv() {
                    if let ContextMenuResult::Selected(0) = result {
                        openxr::ContextMenuResult::ExitSession
                    } else {
                        openxr::ContextMenuResult::Dismissed
                    }
                } else {
                    openxr::ContextMenuResult::Pending
                }
            }
        }

        if openxr::create_instance(false, false).is_ok() {
            let discovery =
                openxr::OpenXrDiscovery::new(Box::new(ContextMenuCallback(embedder_proxy)));
            registry.register(discovery);
        } else {
            let msg =
                "Cannot initialize OpenXR - please ensure runtime is installed and enabled in \
                       the OpenXR developer portal app.\n\nImmersive mode will not function until \
                       this error is fixed.";
            let (sender, _receiver) = ipc::channel().unwrap();
            embedder_proxy.send((
                None,
                EmbedderMsg::Prompt(
                    PromptDefinition::Alert(msg.to_owned(), sender),
                    PromptOrigin::Trusted,
                ),
            ));
        }
    }

    #[cfg(not(feature = "uwp"))]
    fn register_webxr(
        &mut self,
        registry: &mut webxr::MainThreadRegistry,
        _embedder_proxy: EmbedderProxy,
    ) {
        debug!("EmbedderMethods::register_xr");
        if let Some(discovery) = self.xr_discovery.take() {
            registry.register(discovery);
        }
    }

    fn create_event_loop_waker(&mut self) -> Box<dyn EventLoopWaker> {
        debug!("EmbedderMethods::create_event_loop_waker");
        self.waker.clone()
    }
}
