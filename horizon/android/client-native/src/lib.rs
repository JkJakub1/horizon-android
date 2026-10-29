#[cfg(target_os = "android")]
use winit::platform::android::activity::AndroidApp;

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_tag("HorizonClient")
            .with_max_level(log::LevelFilter::Info),
    );
    log::info!("ANDROID_CLIENT start; launching the Horizon client");
    if let Err(error) = horizon_client::run_android(app) {
        log::error!("ANDROID_CLIENT failed: {error}");
    }
}

#[cfg(not(target_os = "android"))]
compile_error!("the Horizon Android client library is built for Android only");
