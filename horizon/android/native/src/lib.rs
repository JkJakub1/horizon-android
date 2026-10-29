#[cfg(target_os = "android")]
use winit::platform::android::activity::AndroidApp;

#[cfg(target_os = "android")]
mod render;

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub fn android_main(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_tag("HorizonAndroid")
            .with_max_level(log::LevelFilter::Info),
    );
    log::info!("ANDROID_RENDER start; this APK is a renderer smoke test, not gameplay");
    if let Err(error) = render::run(app) {
        log::error!("ANDROID_RENDER failed: {error}");
    }
}

#[cfg(not(target_os = "android"))]
compile_error!("the Horizon Android native library is built for Android only");
