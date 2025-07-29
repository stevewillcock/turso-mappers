use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, Field, Ident, Type};

fn impl_try_from_row(ast: DeriveInput) -> proc_macro2::TokenStream {
    let ident: Ident = ast.ident;

    let mut fields: Vec<Field> = vec![];

    match ast.data {
        syn::Data::Struct(data) => {
            for field in data.fields {
                if field.ident.is_some() {
                    fields.push(field)
                }
            }
        }
        _ => panic!("turso_mappers::TryFromRow only supports structs"),
    };

    let field_mappers: Vec<proc_macro2::TokenStream> = fields
        .into_iter()
        .enumerate()
        .map(|(idx, field)| {
            let f_ident = field.ident.unwrap();
            let f_type = field.ty.clone();

            // Check if the field is an Option<T>
            if let Some(inner_type) = get_option_inner_type(&f_type) {
                // Handle Option<T> types
                return match inner_type.as_str() {
                    "i64" => quote! {
                        #f_ident: match row.get_value(#idx) {
                            Ok(value) => match value.as_integer() {
                                Some(val) => Some(*val),
                                None => None,
                            },
                            Err(_) => None,
                        }
                    },
                    "String" => quote! {
                        #f_ident: match row.get_value(#idx) {
                            Ok(value) => match value.as_text() {
                                Some(val) => Some(val.clone()),
                                None => None,
                            },
                            Err(_) => None,
                        }
                    },
                    "f64" => quote! {
                        #f_ident: match row.get_value(#idx) {
                            Ok(value) => match value.as_real() {
                                Some(val) => Some(*val),
                                None => None,
                            },
                            Err(_) => None,
                        }
                    },
                    "Vec<u8>" => quote! {
                        #f_ident: match row.get_value(#idx) {
                            Ok(value) => match value.as_blob() {
                                Some(val) => Some(val.clone()),
                                None => None,
                            },
                            Err(_) => None,
                        }
                    },
                    _ => {
                        // For unsupported Option<T> types, generate a compile-time error
                        let error_msg = format!("Unsupported Option type: Option<{}>", inner_type);
                        quote! {
                            #f_ident: compile_error!(#error_msg)
                        }
                    }
                };
            }

            // Generate code based on the manual implementation for non-Option types
            let type_path = get_type_path(&f_type);

            // Handle different types based on the field type
            match type_path.as_str() {
                "i64" => quote! {
                    #f_ident: *row
                        .get_value(#idx)?
                        .as_integer()
                        .ok_or_else(|| crate::TursoMapperError::ConversionError(format!("{} is not an integer", stringify!(#f_ident))))?
                },
                "String" => quote! {
                    #f_ident: row
                        .get_value(#idx)?
                        .as_text()
                        .ok_or_else(|| crate::TursoMapperError::ConversionError(format!("{} is not a string", stringify!(#f_ident))))?
                        .clone()
                },
                "f64" => quote! {
                    #f_ident: *row
                        .get_value(#idx)?
                        .as_real()
                        .ok_or_else(|| crate::TursoMapperError::ConversionError(format!("{} is not a real", stringify!(#f_ident))))?
                },
                "Vec<u8>" => quote! {
                    #f_ident: row
                        .get_value(#idx)?
                        .as_blob()
                        .ok_or_else(|| crate::TursoMapperError::ConversionError(format!("{} is not a blob", stringify!(#f_ident))))?
                        .clone()
                },
                _ => {
                    // For unsupported types, generate a compile-time error
                    let error_msg = format!("Unsupported type: {}", type_path);
                    quote! {
                        #f_ident: compile_error!(#error_msg)
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    quote! {
        impl crate::TryFromRow for #ident {
            fn try_from_row(row: turso::Row) -> crate::TursoMapperResult<Self> where Self: Sized {
                Ok(Self {
                    #(#field_mappers,)*
                })
            }
        }
    }
}

// Helper function to extract the type path from a Type
fn get_type_path(ty: &Type) -> String {
    match ty {
        Type::Path(type_path) if !type_path.path.segments.is_empty() => {
            let segment = &type_path.path.segments[0];
            let ident = segment.ident.to_string();

            // Handle generic types
            if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                if !args.args.is_empty() {
                    if let Some(syn::GenericArgument::Type(Type::Path(inner_path))) = args.args.first() {
                        if !inner_path.path.segments.is_empty() {
                            let inner_type = inner_path.path.segments[0].ident.to_string();
                            return format!("{}<{}>", ident, inner_type);
                        }
                    }
                }
            }
            ident
        }
        _ => "unknown".to_string(),
    }
}

// Helper function to extract the inner type of an Option<T>
fn get_option_inner_type(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(type_path) if !type_path.path.segments.is_empty() => {
            let segment = &type_path.path.segments[0];
            let ident = segment.ident.to_string();

            if ident == "Option" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if !args.args.is_empty() {
                        if let Some(syn::GenericArgument::Type(inner_type)) = args.args.first() {
                            return Some(get_type_path(inner_type));
                        }
                    }
                }
            }
            None
        }
        _ => None,
    }
}

#[proc_macro_derive(TryFromRow)]
pub fn try_from_row_derive(input: TokenStream) -> TokenStream {
    let ast: DeriveInput = syn::parse(input).unwrap();
    impl_try_from_row(ast).into()
}