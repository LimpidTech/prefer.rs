//! Derive macros for the prefer configuration library.
//!
//! This crate provides the `#[derive(FromValue)]` macro for automatically
//! implementing the `FromValue` trait on structs and enums.
//!
//! # Example
//!
//! ```ignore
//! use prefer::FromValue;
//! use prefer_derive::FromValue;
//!
//! #[derive(FromValue)]
//! struct ServerConfig {
//!     host: String,
//!     port: u16,
//!     #[prefer(default = "false")]
//!     debug: bool,
//! }
//! ```

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, Attribute, Data, DeriveInput, Error, Fields, Ident, Type};

/// Derive the `FromValue` trait for a struct or enum.
///
/// # Attributes
///
/// ## Field Attributes
///
/// - `#[prefer(rename = "name")]` - Use a different key name in the config
/// - `#[prefer(default)]` - Use `Default::default()` if the field is missing
/// - `#[prefer(default = "value")]` - Use a literal value if the field is missing
/// - `#[prefer(skip)]` - Skip this field during deserialization (requires Default)
/// - `#[prefer(flatten)]` - Flatten a nested struct into the parent
///
/// ## Container Attributes (for enums)
///
/// - `#[prefer(tag = "type")]` - Use internally tagged representation
///
/// # Examples
///
/// ```ignore
/// use prefer_derive::FromValue;
///
/// #[derive(FromValue)]
/// struct DatabaseConfig {
///     host: String,
///     #[prefer(default = "5432")]
///     port: u16,
///     #[prefer(rename = "database_name")]
///     name: String,
///     #[prefer(skip)]
///     connection_pool: Option<Pool>,
/// }
///
/// #[derive(FromValue)]
/// #[prefer(tag = "type")]
/// enum Backend {
///     #[prefer(rename = "postgresql")]
///     Postgres { host: String, port: u16 },
///     Sqlite { path: String },
/// }
/// ```
#[proc_macro_derive(FromValue, attributes(prefer))]
pub fn derive_from_value(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match derive_from_value_impl(input) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn derive_from_value_impl(input: DeriveInput) -> Result<TokenStream2, Error> {
    let name = &input.ident;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    match &input.data {
        Data::Struct(data) => derive_struct(name, impl_generics, ty_generics, where_clause, data),
        Data::Enum(data) => {
            let container_attrs = parse_container_attrs(&input.attrs)?;
            derive_enum(
                name,
                impl_generics,
                ty_generics,
                where_clause,
                data,
                container_attrs,
            )
        }
        Data::Union(_) => Err(Error::new_spanned(
            name,
            "FromValue cannot be derived for unions",
        )),
    }
}

#[derive(Default)]
struct ContainerAttrs {
    tag: Option<String>,
}

#[derive(Default)]
struct FieldAttrs {
    rename: Option<String>,
    default: Option<DefaultValue>,
    skip: bool,
    flatten: bool,
    required: bool,
}

enum DefaultValue {
    Default,
    Literal(String),
}

fn parse_container_attrs(attrs: &[Attribute]) -> Result<ContainerAttrs, Error> {
    let mut container = ContainerAttrs::default();

    for attr in attrs {
        if !attr.path().is_ident("prefer") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("tag") {
                let value: syn::LitStr = meta.value()?.parse()?;
                container.tag = Some(value.value());
            }
            Ok(())
        })?;
    }

    Ok(container)
}

fn parse_field_attrs(attrs: &[Attribute]) -> Result<FieldAttrs, Error> {
    let mut field_attrs = FieldAttrs::default();

    for attr in attrs {
        if !attr.path().is_ident("prefer") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename") {
                let value: syn::LitStr = meta.value()?.parse()?;
                field_attrs.rename = Some(value.value());
            } else if meta.path.is_ident("default") {
                if meta.input.peek(syn::Token![=]) {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    field_attrs.default = Some(DefaultValue::Literal(value.value()));
                } else {
                    field_attrs.default = Some(DefaultValue::Default);
                }
            } else if meta.path.is_ident("skip") {
                field_attrs.skip = true;
            } else if meta.path.is_ident("flatten") {
                field_attrs.flatten = true;
            } else if meta.path.is_ident("required") {
                field_attrs.required = true;
            }
            Ok(())
        })?;
    }

    Ok(field_attrs)
}

fn derive_struct(
    name: &Ident,
    impl_generics: syn::ImplGenerics,
    ty_generics: syn::TypeGenerics,
    where_clause: Option<&syn::WhereClause>,
    data: &syn::DataStruct,
) -> Result<TokenStream2, Error> {
    let fields = match &data.fields {
        Fields::Named(fields) => &fields.named,
        Fields::Unnamed(_) => {
            return Err(Error::new_spanned(
                name,
                "FromValue cannot be derived for tuple structs",
            ))
        }
        Fields::Unit => {
            return Ok(quote! {
                impl #impl_generics prefer::FromValue for #name #ty_generics #where_clause {
                    fn from_value(_value: &prefer::ConfigValue) -> prefer::Result<Self> {
                        Ok(Self)
                    }
                }
            });
        }
    };

    let mut field_extractions = Vec::new();

    for field in fields {
        let field_name = field.ident.as_ref().unwrap();
        let field_type = &field.ty;
        let attrs = parse_field_attrs(&field.attrs)?;

        let key_name = attrs
            .rename
            .clone()
            .unwrap_or_else(|| field_name.to_string());

        let extraction = if attrs.skip {
            quote! {
                #field_name: ::core::default::Default::default()
            }
        } else if attrs.flatten {
            quote! {
                #field_name: <#field_type as prefer::FromValue>::from_value(value)?
            }
        } else if attrs.required {
            // Required fields must always be present, even if Option type
            quote! {
                #field_name: <#field_type as prefer::FromValue>::from_value(
                    obj.get(#key_name).ok_or_else(|| prefer::Error::KeyNotFound(#key_name.to_string()))?
                ).map_err(|e| e.with_key(#key_name))?
            }
        } else {
            match &attrs.default {
                Some(DefaultValue::Default) => {
                    quote! {
                        #field_name: obj.get(#key_name)
                            .map(|v| <#field_type as prefer::FromValue>::from_value(v))
                            .transpose()
                            .map_err(|e| e.with_key(#key_name))?
                            .unwrap_or_default()
                    }
                }
                Some(DefaultValue::Literal(lit)) => {
                    let default_expr = generate_default_expr(field_type, lit)?;
                    quote! {
                        #field_name: obj.get(#key_name)
                            .map(|v| <#field_type as prefer::FromValue>::from_value(v))
                            .transpose()
                            .map_err(|e| e.with_key(#key_name))?
                            .unwrap_or_else(|| #default_expr)
                    }
                }
                None => {
                    if is_option_type(field_type) {
                        quote! {
                            #field_name: obj.get(#key_name)
                                .map(|v| <#field_type as prefer::FromValue>::from_value(v))
                                .transpose()
                                .map_err(|e| e.with_key(#key_name))?
                                .flatten()
                        }
                    } else {
                        quote! {
                            #field_name: <#field_type as prefer::FromValue>::from_value(
                                obj.get(#key_name).ok_or_else(|| prefer::Error::KeyNotFound(#key_name.to_string()))?
                            ).map_err(|e| e.with_key(#key_name))?
                        }
                    }
                }
            }
        };

        field_extractions.push(extraction);
    }

    let type_name = name.to_string();

    Ok(quote! {
        impl #impl_generics prefer::FromValue for #name #ty_generics #where_clause {
            fn from_value(value: &prefer::ConfigValue) -> prefer::Result<Self> {
                let obj = value.as_object().ok_or_else(|| prefer::Error::ConversionError {
                    key: String::new(),
                    type_name: #type_name.to_string(),
                    source: "expected object".into(),
                })?;

                Ok(Self {
                    #(#field_extractions),*
                })
            }
        }
    })
}

fn derive_enum(
    name: &Ident,
    impl_generics: syn::ImplGenerics,
    ty_generics: syn::TypeGenerics,
    where_clause: Option<&syn::WhereClause>,
    data: &syn::DataEnum,
    container_attrs: ContainerAttrs,
) -> Result<TokenStream2, Error> {
    let type_name = name.to_string();

    if let Some(tag_field) = container_attrs.tag {
        // Internally tagged enum
        let mut variant_matches = Vec::new();

        for variant in &data.variants {
            let variant_name = &variant.ident;
            let attrs = parse_field_attrs(&variant.attrs)?;
            let tag_value = attrs
                .rename
                .clone()
                .unwrap_or_else(|| variant_name.to_string());

            let construction = match &variant.fields {
                Fields::Named(fields) => {
                    let mut field_extractions = Vec::new();
                    for field in &fields.named {
                        let field_name = field.ident.as_ref().unwrap();
                        let field_type = &field.ty;
                        let field_attrs = parse_field_attrs(&field.attrs)?;
                        let key_name = field_attrs
                            .rename
                            .clone()
                            .unwrap_or_else(|| field_name.to_string());

                        let extraction = if field_attrs.skip {
                            quote! { #field_name: ::core::default::Default::default() }
                        } else if let Some(DefaultValue::Default) = field_attrs.default {
                            quote! {
                                #field_name: obj.get(#key_name)
                                    .map(|v| <#field_type as prefer::FromValue>::from_value(v))
                                    .transpose()?
                                    .unwrap_or_default()
                            }
                        } else if let Some(DefaultValue::Literal(lit)) = &field_attrs.default {
                            let default_expr = generate_default_expr(field_type, lit)?;
                            quote! {
                                #field_name: obj.get(#key_name)
                                    .map(|v| <#field_type as prefer::FromValue>::from_value(v))
                                    .transpose()?
                                    .unwrap_or_else(|| #default_expr)
                            }
                        } else if is_option_type(field_type) {
                            quote! {
                                #field_name: obj.get(#key_name)
                                    .map(|v| <#field_type as prefer::FromValue>::from_value(v))
                                    .transpose()?
                                    .flatten()
                            }
                        } else {
                            quote! {
                                #field_name: <#field_type as prefer::FromValue>::from_value(
                                    obj.get(#key_name).ok_or_else(|| prefer::Error::KeyNotFound(#key_name.to_string()))?
                                )?
                            }
                        };
                        field_extractions.push(extraction);
                    }
                    quote! { Self::#variant_name { #(#field_extractions),* } }
                }
                Fields::Unnamed(fields) => {
                    if fields.unnamed.len() == 1 {
                        let field_type = &fields.unnamed.first().unwrap().ty;
                        quote! {
                            Self::#variant_name(<#field_type as prefer::FromValue>::from_value(value)?)
                        }
                    } else {
                        return Err(Error::new_spanned(
                            variant,
                            "Tuple variants with multiple fields are not supported",
                        ));
                    }
                }
                Fields::Unit => {
                    quote! { Self::#variant_name }
                }
            };

            variant_matches.push(quote! {
                #tag_value => { #construction }
            });
        }

        Ok(quote! {
            impl #impl_generics prefer::FromValue for #name #ty_generics #where_clause {
                fn from_value(value: &prefer::ConfigValue) -> prefer::Result<Self> {
                    let obj = value.as_object().ok_or_else(|| prefer::Error::ConversionError {
                        key: String::new(),
                        type_name: #type_name.to_string(),
                        source: "expected object".into(),
                    })?;

                    let tag = obj.get(#tag_field)
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| prefer::Error::ConversionError {
                            key: #tag_field.to_string(),
                            type_name: #type_name.to_string(),
                            source: "missing or invalid tag field".into(),
                        })?;

                    Ok(match tag {
                        #(#variant_matches)*
                        other => return Err(prefer::Error::ConversionError {
                            key: #tag_field.to_string(),
                            type_name: #type_name.to_string(),
                            source: format!("unknown variant: {}", other).into(),
                        }),
                    })
                }
            }
        })
    } else {
        // Untagged enum - try each variant
        let mut variant_attempts = Vec::new();

        for variant in &data.variants {
            let variant_name = &variant.ident;
            let attrs = parse_field_attrs(&variant.attrs)?;
            let key_name = attrs
                .rename
                .clone()
                .unwrap_or_else(|| variant_name.to_string());

            let attempt = match &variant.fields {
                Fields::Named(fields) => {
                    let mut field_extractions = Vec::new();
                    for field in &fields.named {
                        let field_name = field.ident.as_ref().unwrap();
                        let field_type = &field.ty;
                        let field_attrs = parse_field_attrs(&field.attrs)?;
                        let field_key = field_attrs
                            .rename
                            .clone()
                            .unwrap_or_else(|| field_name.to_string());

                        let extraction = if field_attrs.skip {
                            quote! { #field_name: ::core::default::Default::default() }
                        } else {
                            quote! {
                                #field_name: <#field_type as prefer::FromValue>::from_value(
                                    inner.get(#field_key)?
                                )?
                            }
                        };
                        field_extractions.push(extraction);
                    }
                    quote! {
                        if let Some(inner) = obj.get(#key_name).and_then(|v| v.as_object()) {
                            return Ok(Self::#variant_name { #(#field_extractions),* });
                        }
                    }
                }
                Fields::Unnamed(fields) => {
                    if fields.unnamed.len() == 1 {
                        let field_type = &fields.unnamed.first().unwrap().ty;
                        quote! {
                            if let Some(inner) = obj.get(#key_name) {
                                if let Ok(val) = <#field_type as prefer::FromValue>::from_value(inner) {
                                    return Ok(Self::#variant_name(val));
                                }
                            }
                        }
                    } else {
                        return Err(Error::new_spanned(
                            variant,
                            "Tuple variants with multiple fields are not supported",
                        ));
                    }
                }
                Fields::Unit => {
                    quote! {
                        if obj.contains_key(#key_name) {
                            return Ok(Self::#variant_name);
                        }
                    }
                }
            };

            variant_attempts.push(attempt);
        }

        Ok(quote! {
            impl #impl_generics prefer::FromValue for #name #ty_generics #where_clause {
                fn from_value(value: &prefer::ConfigValue) -> prefer::Result<Self> {
                    let obj = value.as_object().ok_or_else(|| prefer::Error::ConversionError {
                        key: String::new(),
                        type_name: #type_name.to_string(),
                        source: ("expected object"),
                    })?;

                    #(#variant_attempts)*

                    Err(prefer::Error::ConversionError {
                        key: String::new(),
                        type_name: #type_name.to_string(),
                        source: ("no matching variant found"),
                    })
                }
            }
        })
    }
}

fn is_option_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            return segment.ident == "Option";
        }
    }
    false
}

fn generate_default_expr(_ty: &Type, literal: &str) -> Result<TokenStream2, Error> {
    // Try to parse as different literal types
    if let Ok(n) = literal.parse::<i64>() {
        return Ok(quote! { #n as _ });
    }
    if let Ok(n) = literal.parse::<f64>() {
        return Ok(quote! { #n as _ });
    }
    if literal == "true" {
        return Ok(quote! { true });
    }
    if literal == "false" {
        return Ok(quote! { false });
    }
    // Default to string
    Ok(quote! { #literal.to_string().parse().unwrap_or_default() })
}
