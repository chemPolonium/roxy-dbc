//! 把 `roxy-dbc.ico` 嵌进 exe 的 PE 资源，资源管理器与任务栏上显示的就是它。
//! 窗口自己的图标是运行时由 `src/icon.rs` 解同一个文件交给 winit。

fn main() {
    println!("cargo:rerun-if-changed=roxy-dbc.ico");
    println!("cargo:rerun-if-changed=build.rs");

    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("roxy-dbc.ico");
        if let Err(error) = resource.compile() {
            println!("cargo:warning=没能嵌入 exe 图标: {error}");
        }
    }
}
