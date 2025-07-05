use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, Field, Ident, Type};

fn impl_try_from_row(ast: DeriveInput) -> proc_macro2::TokenStream {
    let ident: Ident = ast.ident;

    let mut fields: Vec<Field> = vec![];

    match ast.data {
        syn::Data::Struct(data) => {
            for field in data.fields {
                if (field.ident).is_some() {
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
            let f_type = field.ty;

            // Generate code based on the manual implementation
            let type_path = get_type_path(&f_type);

            // Handle different types based on the field type
            if type_path == "i64" {
                quote! {
                    #f_ident: *row
                        .get_value(#idx)?
                        .as_integer()
                        .ok_or_else(|| crate::TursoMapperError::ConversionError(format!("{} is not an integer", stringify!(#f_ident))))?
                }
            } else if type_path == "String" {
                quote! {
                    #f_ident: row
                        .get_value(#idx)?
                        .as_text()
                        .ok_or_else(|| crate::TursoMapperError::ConversionError(format!("{} is not a string", stringify!(#f_ident))))?
                        .clone()
                }
            } else {
                // For unsupported types, generate a compile-time error
                let error_msg = format!("Unsupported type: {}", type_path);
                quote! {
                    #f_ident: compile_error!(#error_msg)
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
            segment.ident.to_string()
        }
        _ => "unknown".to_string(),
    }
}

#[proc_macro_derive(TryFromRow)]
pub fn try_from_row_derive(input: TokenStream) -> TokenStream {
    let ast: DeriveInput = syn::parse(input).unwrap();
    impl_try_from_row(ast).into()
}
