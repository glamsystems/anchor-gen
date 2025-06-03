use anchor_syn::idl::types::IdlAccountItem;
use heck::{ToPascalCase, ToSnakeCase};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::GlamIxCodeGenConfig;

/// Generates a list of [IdlAccountItem]s as a [TokenStream].
pub fn generate_account_fields(
    name: &str,
    accounts: &[IdlAccountItem],
) -> (TokenStream, TokenStream) {
    let mut all_structs: Vec<TokenStream> = vec![];
    let all_fields = accounts
        .iter()
        .map(|account| match account {
            anchor_syn::idl::types::IdlAccountItem::IdlAccount(info) => {
                let acc_name = format_ident!("{}", info.name.to_snake_case());
                let annotation = if info.is_mut {
                    quote! { #[account(mut)] }
                } else {
                    quote! {}
                };
                let acc_type = if info.is_signer {
                    quote! { Signer<'info> }
                } else {
                    quote! { AccountInfo<'info> }
                };
                let acc_type = if info.is_optional.is_some() {
                    quote! { Option<#acc_type> }
                } else {
                    acc_type
                };
                quote! {
                   #annotation
                   pub #acc_name: #acc_type
                }
            }
            anchor_syn::idl::types::IdlAccountItem::IdlAccounts(inner) => {
                let field_name = format_ident!("{}_{}", name, inner.name.to_snake_case());
                let sub_name = format!("{}{}", name, inner.name.to_pascal_case());
                let sub_ident = format_ident!("{}", &sub_name);
                let (sub_structs, sub_fields) = generate_account_fields(&sub_name, &inner.accounts);
                all_structs.push(sub_structs);
                all_structs.push(quote! {
                    #[derive(Accounts)]
                    pub struct #sub_ident<'info> {
                        #sub_fields
                    }
                });
                quote! {
                    pub #field_name: #sub_ident<'info>
                }
            }
        })
        .collect::<Vec<_>>();
    (
        quote! {
            #(#all_structs)*
        },
        quote! {
            #(#all_fields),*
        },
    )
}

pub fn generate_glam_account_fields(
    name: &str,
    accounts: &[IdlAccountItem],
    ix_code_gen_config: Option<&GlamIxCodeGenConfig>,
    vec_accounts_ts: &mut Vec<TokenStream>,
    accounts_to_keep: &mut Vec<String>,
    all_accounts: &mut Vec<String>,
    map_sub_accounts: &mut std::collections::HashMap<String, Vec<(String, bool)>>,
    sub_accounts_struct_name: String,
) -> (TokenStream, TokenStream) {
    let vault_aliases =
        ix_code_gen_config.map_or(Vec::new(), |c| c.vault_aliases.clone().unwrap_or_default());
    let signer_aliases =
        ix_code_gen_config.map_or(Vec::new(), |c| c.signer_aliases.clone().unwrap_or_default());

    let mut all_structs: Vec<TokenStream> = vec![];

    let all_fields = accounts
        .iter()
        .map(|account| match account {
            anchor_syn::idl::types::IdlAccountItem::IdlAccount(info) => {
                // account annotation
                let mut annotation = if info.is_mut {
                    quote! { #[account(mut)] }
                } else {
                    quote! {}
                };

                // type and lifetime
                // always remove signer if it's a vault alias
                let acc_type = if info.is_signer {
                    quote! { Signer<'info> }
                } else if info.name.to_snake_case().eq("system_program") {
                    quote! { Program<'info, System> }
                } else if info.name == "rent" {
                    quote! { Sysvar<'info, Rent> }
                } else {
                    let mut ts = quote! {
                        /// CHECK: should be validated by target program
                    };
                    ts.extend(annotation);
                    annotation = ts;

                    quote! { AccountInfo<'info> }
                };

                let acc_type = if info.is_optional.is_some() {
                    quote! { Option<#acc_type> }
                } else {
                    acc_type
                };

                let acc_name = format_ident!("{}", info.name.to_snake_case());

                // result
                let is_optional = info.is_optional.unwrap_or(false);
                all_accounts.push(info.name.to_snake_case());
                if let Some(sub) = map_sub_accounts.get_mut(&sub_accounts_struct_name) {
                    sub.push((info.name.to_snake_case(), is_optional));
                } else {
                    map_sub_accounts.insert(
                        sub_accounts_struct_name.clone(),
                        vec![(info.name.to_snake_case(), is_optional)],
                    );
                }
                if vault_aliases.contains(&info.name.to_snake_case()) {
                    None
                } else if signer_aliases.contains(&info.name.to_snake_case()) {
                    None
                } else {
                    accounts_to_keep.push(info.name.to_snake_case());

                    Some(quote! {
                       #annotation
                       pub #acc_name: #acc_type
                    })
                }
            }
            anchor_syn::idl::types::IdlAccountItem::IdlAccounts(inner) => {
                let field_name = format_ident!("{}_{}", name, inner.name.to_snake_case());
                let sub_name = format!("{}{}", name, inner.name.to_pascal_case());
                let sub_ident = format_ident!("{}", &sub_name);

                let (sub_structs, sub_fields) = generate_glam_account_fields(
                    &sub_name,
                    &inner.accounts,
                    ix_code_gen_config,
                    vec_accounts_ts,
                    accounts_to_keep,
                    all_accounts,
                    map_sub_accounts,
                    field_name.to_string(),
                );
                all_structs.push(sub_structs);
                all_structs.push(quote! {
                    #[derive(Accounts)]
                    pub struct #sub_ident<'info> {
                        #sub_fields
                    }
                });
                // All sub accounts will be flattened in the parent struct, we don't need to add the sub struct
                // Some(quote! {
                //     pub #field_name: #sub_ident<'info>
                // })
                None
            }
        })
        .filter(|x| x.is_some())
        .map(|x| x.unwrap())
        .collect::<Vec<_>>();

    vec_accounts_ts.extend(all_fields.clone());

    (
        quote! {
            #(#all_structs)*
        },
        quote! {
            #(#all_fields),*
        },
    )
}
