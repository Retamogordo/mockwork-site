
#[macro_export]
macro_rules! block_on_target {
	($fut: expr) => {
        #[cfg(not(target_arch = "wasm32"))]
		block_on($fut);
        #[cfg(not(target_arch = "wasm32"))]
        ();
	}
}

#[cfg(target_arch = "wasm32")]
pub const TASK_SWITCH_DELAY_MILLIS: u64 = 4; 
#[cfg(not(target_arch = "wasm32"))]
pub const TASK_SWITCH_DELAY_MILLIS: u64 = 0; 

#[cfg(not(target_arch = "wasm32"))]
type JoinHandleType<T> = async_std::task::JoinHandle<T>;
#[cfg(target_arch = "wasm32")]
type JoinHandleType<T> = futures::future::Ready<T>;

pub struct TargetDependantJoinHandle<T>(JoinHandleType<T>);

impl<T> TargetDependantJoinHandle<T> {
/*    pub fn handle(self) -> JoinHandleType<T> {
        self.0
    }*/
}
  
#[cfg(not(target_arch = "wasm32"))]
pub fn spawn<F, T>(f: F) -> TargetDependantJoinHandle<T>
where
    F: async_std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static, {

        TargetDependantJoinHandle(async_std::task::spawn(f))
}

#[cfg(target_arch = "wasm32")]
pub fn spawn<F>(f: F) -> TargetDependantJoinHandle<()>
where
    F: async_std::future::Future<Output = ()> + 'static {
        wasm_bindgen_futures::spawn_local(f);
        TargetDependantJoinHandle(futures::future::ready(()))
}


use futures::channel::{oneshot};
use futures::{pin_mut, select, FutureExt};

pub struct CancellableRun {
    cancel_signal: (oneshot::Sender<()>, Option<oneshot::Receiver<()>>), 
    done_response: (Option<oneshot::Sender<()>>, oneshot::Receiver<()>),   
}

impl CancellableRun {
    fn new() -> Self {
        let cancel_signal = oneshot::channel(); 
        let done_response = oneshot::channel(); 

        Self { 
            cancel_signal: (cancel_signal.0, Some(cancel_signal.1)), 
            done_response: (Some(done_response.0), done_response.1) 
        }      
    }
    pub async fn cancel(self)  {
//        if let Ok(_) = self.cancel_signal.0.send(()) {
        let _ = self.cancel_signal.0.send(());
        let _ = self.done_response.1.await;
//        }
    }
/*    pub async fn while_alive(self) {
        let _ = self.done_response.1.await;
    }*/ 
}
/*
impl Drop for CancellableRun {
    fn drop(&mut self) {
        log::info!("dropping cancelable");
 //       self.cancel_signal.0.send(());
    }
}
*/

async fn cancellable_run<F>(f: F, cancel_signal: oneshot::Receiver<()>) 
    where F: async_std::future::Future<Output = ()> + 'static {
    
    let func_run = f.fuse();
    let cancel_task = cancel_signal.fuse();

    pin_mut!(func_run, cancel_task);

    select! {
        () = func_run => (),
        _ = cancel_task => {
//            log::info!("on cancel task");
        }
    }
}

//#[cfg(target_arch = "wasm32")]
pub fn spawn_cancellable<F>(f: F, on_done: impl FnOnce() + 'static) -> CancellableRun
where
    F: async_std::future::Future<Output = ()> + 'static {
        let mut cancellable = CancellableRun::new();
        let cancel_signal = cancellable.cancel_signal.1.take().unwrap();
        let done_tx = cancellable.done_response.0.take().unwrap();

        self::spawn(
            async move {

            cancellable_run(f, cancel_signal).await;
            
            let _ = done_tx.send(());
            on_done();
        });
        cancellable
}


#[cfg(not(target_arch = "wasm32"))]
pub fn spawn_default_failed_test() -> TargetDependantJoinHandle<bool> {

    spawn(async { false })
}

/*
#[cfg(target_arch = "wasm32")]
pub fn spawn_default_failed_test() -> TargetDependantJoinHandle<()> {
    spawn(async { () })
}
*/
#[cfg(not(target_arch = "wasm32"))]
pub fn delay(d: core::time::Duration) -> futures_timer::Delay {
    futures_timer::Delay::new(d) 
}

#[cfg(target_arch = "wasm32")]
pub fn delay(d: core::time::Duration) -> gloo_timers::future::TimeoutFuture {
    gloo_timers::future::TimeoutFuture::new(d.as_millis() as u32)
}

pub async fn run_on_next_js_tick() {
    #[cfg(target_arch = "wasm32")]
    self::delay(core::time::Duration::from_millis(0)).await;
}
