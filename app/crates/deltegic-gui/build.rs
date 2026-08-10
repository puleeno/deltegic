fn main() {
    // Compile only main.slint — captcha.slint is imported from within main.slint.
    // Calling compile() twice would overwrite SLINT_INCLUDE_GENERATED and lose the
    // first file's types in include_modules!().
    slint_build::compile("ui/main.slint").expect("Slint compilation failed");
}
