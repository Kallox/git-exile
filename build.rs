//! Incrusta el icono en el ejecutable de Windows, el que muestra el explorador.

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    #[cfg(windows)]
    winresource::WindowsResource::new()
        .set_icon("assets/icon.ico")
        .compile()
        .expect("no se pudo incrustar el icono");
}
