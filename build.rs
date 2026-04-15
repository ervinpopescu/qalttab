fn main() {
    #[cfg(not(target_os = "linux"))]
    compile_error!("qalttab only supports Linux");
}
