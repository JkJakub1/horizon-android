# Horizon Android builds

This project contains two separate ARM64 NativeActivity APKs: the known-good Vulkan render check and an early APK wired to Horizon's actual Rust client.

The render-check APK is only a diagnostic; it does not load a world or use the game's renderer. The client APK launches `horizon-client` through its existing `winit` event loop and `horizon-client::gfx::Renderer`. Neither APK bundles Minecraft resources or server JARs. PojavLauncher is not used.

## Architecture

- Android starts `android.app.NativeActivity` from `app/src/main/AndroidManifest.xml`; its library name is `horizon_android`.
- `android/native` builds the render-check library; `android/client-native` builds the game library. Their separate Gradle modules, native library names, and application IDs keep the smoke-test APK independently buildable and installable.
- The client APK's `android_main(AndroidApp)` initializes Android logging and calls `horizon_client::run_android`. The client stores files in its app-specific external files directory; Android's activity lifecycle, surface, and input are forwarded by winit.
- winit receives the `AndroidApp` through `EventLoopBuilderExtAndroid::with_android_app`. Its resumed/suspended/window events drive the client window and existing renderer.
- The smoke check is Vulkan-only. The game retains its existing renderer and graphics-backend selection.
- The diagnostic uses the same pinned winit and wgpu versions as Horizon. It is intentionally separate from `horizon-client::gfx::Renderer`.

## Build the debug APK on Linux x86-64

Toolchain baseline: Rust 1.95.0, JDK 17, Gradle 8.13, Android Gradle Plugin 8.7.2, SDK Platform/Build Tools 35, and NDK 27.2.12479018. Rust 1.95.0 is required by the current Horizon workspace (`horizon-mods`); it is newer than the standalone probe's original Rust baseline.

Install the Android Rust target once:

```sh
rustup toolchain install 1.95.0 --profile minimal
rustup target add aarch64-linux-android --toolchain 1.95.0
```

Set the Android/JDK environment and build:

```sh
export JAVA_HOME="$HOME/.local/jdks/temurin-17.0.13+11"
export ANDROID_HOME="$HOME/Android/Sdk"
export ANDROID_SDK_ROOT="$ANDROID_HOME"
export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/27.2.12479018"
export ANDROID_NDK_ROOT="$ANDROID_NDK_HOME"
export PATH="$JAVA_HOME/bin:$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$HOME/.cargo/bin:$PATH"

sdkmanager --licenses
sdkmanager --install "platforms;android-35" "build-tools;35.0.0" "ndk;27.2.12479018"

cd horizon/android
./gradlew --no-daemon --stacktrace :app:assembleDebug :clientApp:assembleDebug
```

Gradle runs the pinned Rust toolchain for `aarch64-linux-android`, uses the NDK's API-26 linker, and copies each native library into that module's generated build output before packaging.

The standalone render-check APK is:

```text
horizon/android/app/build/outputs/apk/debug/app-debug.apk
```

The actual-client APK is:

```text
horizon/android/clientApp/build/outputs/apk/debug/clientApp-debug.apk
```

The client uses Horizon's existing `fast` Rust profile to avoid the high memory cost of release LTO when compiling the large client crate.

## Install and test on a physical Android device (Windows PowerShell)

Use an arm64 device with Android API 26+ and Vulkan 1.0 support. From the repository root, install and launch:

```powershell
$ErrorActionPreference = 'Stop'
$apk = (Resolve-Path .\horizon\android\clientApp\build\outputs\apk\debug\clientApp-debug.apk).Path
adb devices -l
adb install -r $apk
adb shell am force-stop dev.playhorizon.horizon.client
adb logcat -c
adb shell am start -n dev.playhorizon.horizon.client/android.app.NativeActivity
adb logcat -v threadtime -s HorizonClient:I AndroidRuntime:E
```

Without resources the client should present a resource-setup screen; this proves client startup and rendering only, not gameplay. Prepare resources on a PC from your own licensed Minecraft installation and your own 26.3 server JAR, following [the resource preparation instructions](../docs/native-client.md#run-from-a-checkout). Do not copy the server JAR itself. From the repository root, copy the prepared directory to the app-specific files folder:

```powershell
$prepared = (Resolve-Path .\horizon\.local\horizon-client\26.3).Path
$destination = '/sdcard/Android/data/dev.playhorizon.horizon.client/files/resources'
adb shell "mkdir -p $destination"
adb push $prepared $destination
adb shell am force-stop dev.playhorizon.horizon.client
adb shell am start -n dev.playhorizon.horizon.client/android.app.NativeActivity
adb logcat -v threadtime -s HorizonClient:I AndroidRuntime:E
```

After import, look for `Horizon: resources loaded` and the title screen. Entering a local world must be tested separately before claiming gameplay works. Audio output, touch controls, gamepad input, suspend/resume, and real-device gameplay have not yet been validated.

To retest the separate diagnostic APK:

```powershell
$apk = (Resolve-Path .\horizon\android\app\build\outputs\apk\debug\app-debug.apk).Path
adb install -r $apk
adb shell am start -n dev.playhorizon.horizon/android.app.NativeActivity
adb logcat -d -v threadtime -s HorizonAndroid:I AndroidRuntime:E |
    Tee-Object -FilePath .\horizon-android-render.log
@(
    "manufacturer=$(adb shell getprop ro.product.manufacturer)"
    "model=$(adb shell getprop ro.product.model)"
    "android=$(adb shell getprop ro.build.version.release)"
    "api=$(adb shell getprop ro.build.version.sdk)"
    "abi=$(adb shell getprop ro.product.cpu.abi)"
) | Tee-Object -FilePath .\horizon-android-device.txt
adb shell screencap -p /sdcard/horizon-android-render.png
adb pull /sdcard/horizon-android-render.png .\horizon-android-render.png
```

A successful smoke render must show the diagnostic frame and matching `PRESENTED_FRAME` evidence identifying the Vulkan backend, adapter, dimensions, and format. For the client, the `ANDROID_CLIENT start` log, rendered resource-setup screen, rendered title screen after import, and successful world entry are separate checkpoints. APK compilation or Activity launch alone proves none of the later checkpoints.

The APK output above is separate from `/workspaces/horizon-android/app-debug.apk`, the already-tested standalone probe APK; this build does not overwrite that file.

## Current limitations

- The Android clipboard path uses the Android clipboard manager; physical clipboard behavior is unverified.
- Android controller input and DualSense HID output are not yet supported by Horizon's current `gilrs` backend. Android builds a logged no-HID-output stub for DualSense.
- Touchscreen controls, audio behavior, suspend/resume, resource import through a file picker, and actual local-world gameplay are not yet validated.
- A prior standalone rendering probe ran on a Xiaomi Redmi Note 13, but neither APK's latest build has been tested on that device.
