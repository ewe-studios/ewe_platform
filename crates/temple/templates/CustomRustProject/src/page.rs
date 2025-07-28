use tinytemplate::TinyTemplate;

fn main() {
    let mut tt = TinyTemplate::new();

    println!("ProjectDir: {{ PROJECT_DIRECTORY }}");
    println!("TemplateName: {{ TEMPLATE_NAME }}");
    println!("PackageName: {{ PACKAGE_NAME }}");

    println!("RootPackageName: {{ ROOT_PACKAGE_NAME }}");
    println!("RootPustVersion: {{ ROOT_PACKAGE_RUST_VERSION }}");
    println!("RootPackageVersion: {{ ROOT_PACKAGE_VERSION }}");
    println!("RootPackageDocs: {{ ROOT_PACKAGE_DOCUMENTATION }}");
}
