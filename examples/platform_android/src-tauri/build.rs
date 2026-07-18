fn main() {
    // foundation_platform owns the build pipeline on all platforms.
    // Calls tauri_build::build() internally + annotation scanning.
    foundation_platform::codegen::generate_platform_code();
}
