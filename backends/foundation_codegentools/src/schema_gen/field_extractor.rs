use syn::{Fields, GenericArgument, PathArguments, Type};

#[derive(Debug, Clone)]
pub struct StructField {
    pub name: String,
    pub rust_type: String,
    pub nullable: bool,
}

#[must_use]
pub fn extract_fields(item: &syn::ItemStruct) -> Option<Vec<StructField>> {
    let Fields::Named(named) = &item.fields else {
        return None;
    };

    let mut result = Vec::new();
    for field in &named.named {
        let name = field.ident.as_ref()?.to_string();
        let (rust_type, nullable) = resolve_type(&field.ty);
        result.push(StructField {
            name,
            rust_type,
            nullable,
        });
    }
    Some(result)
}

fn resolve_type(ty: &Type) -> (String, bool) {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            let ident = segment.ident.to_string();

            if ident == "Option" {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(inner)) = args.args.first() {
                        let (inner_type, _) = resolve_type(inner);
                        return (inner_type, true);
                    }
                }
                return ("Option".to_string(), true);
            }

            if ident == "Vec" {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(Type::Path(inner_path))) =
                        args.args.first()
                    {
                        if inner_path
                            .path
                            .segments
                            .last()
                            .is_some_and(|s| s.ident == "u8")
                        {
                            return ("Vec<u8>".to_string(), false);
                        }
                    }
                }
                return ("Vec".to_string(), false);
            }

            return (ident, false);
        }
    }
    ("unknown".to_string(), false)
}
