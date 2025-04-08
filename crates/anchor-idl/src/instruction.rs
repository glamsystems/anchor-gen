use anchor_syn::idl::IdlInstruction;
use heck::{ToPascalCase, ToSnakeCase};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;

use crate::GlamIxCodeGenConfig;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct AccuntInfo {
    name: String,
    index: u16,
    writable: bool,
    signer: bool,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct IxInfo {
    src_ix_name: String,
    src_discriminator: [u8; 8],
    #[serde(skip_serializing_if = "Option::is_none")]
    dst_ix_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dst_discriminator: Option<[u8; 8]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dynamic_accounts: Option<Vec<AccuntInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    static_accounts: Option<Vec<AccuntInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    index_map: Option<Vec<i32>>,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct GlamIxRemapping {
    pub program_id: String,
    pub instructions: Vec<IxInfo>,
}

fn compute_discriminator(ix_name: &str) -> [u8; 8] {
    // Format the identifier as per Anchor's convention
    let identifier = format!("global:{}", ix_name);

    // Compute the SHA-256 hash of the identifier
    let mut hasher = Sha256::new();
    hasher.update(identifier.as_bytes());
    let hash = hasher.finalize();

    // Extract the first 8 bytes as the discriminator
    let mut discriminator = [0u8; 8];
    discriminator.copy_from_slice(&hash[..8]);
    discriminator
}

/// Generates a single instruction handler.
pub fn generate_ix_handler(ix: &IdlInstruction) -> TokenStream {
    let ix_name = format_ident!("{}", ix.name.to_snake_case());
    let accounts_name = format_ident!("{}", ix.name.to_pascal_case());

    let args = ix
        .args
        .iter()
        .map(|arg| {
            let name = format_ident!("_{}", arg.name.to_snake_case());
            let type_name = crate::ty_to_rust_type(&arg.ty);
            let stream: proc_macro2::TokenStream = type_name.parse().unwrap();
            quote! {
                #name: #stream
            }
        })
        .collect::<Vec<_>>();

    if cfg!(feature = "compat-program-result") {
        quote! {
            pub fn #ix_name(
                _ctx: Context<#accounts_name>,
                #(#args),*
            ) -> ProgramResult {
                unimplemented!("This program is a wrapper for CPI.")
            }
        }
    } else {
        quote! {
            pub fn #ix_name(
                _ctx: Context<#accounts_name>,
                #(#args),*
            ) -> Result<()> {
                unimplemented!("This program is a wrapper for CPI.")
            }
        }
    }
}

pub fn generate_glam_ix_structs(
    ixs: &[IdlInstruction],
    program_name: &Ident,
    ixs_to_generate: &[String],
    ix_code_gen_configs: &std::collections::HashMap<String, GlamIxCodeGenConfig>,
) -> (
    TokenStream,
    Vec<IxInfo>,
    HashMap<String, HashMap<String, Vec<String>>>,
) {
    //  ixs_to_generate &&  ix_code_gen_configs: generate only the intersecting instructions
    // !ixs_to_generate && !ix_code_gen_configs: generate all instructions
    // !ixs_to_generate &&  ix_code_gen_configs: generate only the instructions specified in the config
    //  ixs_to_generate && !ix_code_gen_configs: generate only the specified instructions

    // Multiple ixs might share the same accounts struct, so we need to keep track of which ones have been generated
    let mut accounts_structs_generated: Vec<String> = vec![];
    let mut ix_infos: Vec<IxInfo> = vec![];

    let mut ixs_sub_accounts: HashMap<String, HashMap<String, Vec<String>>> = HashMap::new();

    let defs = ixs
        .iter()
        .filter(|ix| ixs_to_generate.is_empty() || ixs_to_generate.contains(&ix.name.to_string()))
        .map(|ix| {
            let accounts_struct_name_pascal_case = if let Some(accounts_struct) =
                ix_code_gen_configs
                    .get(ix.name.as_str())
                    .unwrap_or(&GlamIxCodeGenConfig::default())
                    .accounts_struct
                    .clone()
            {
                accounts_struct.to_pascal_case()
            } else {
                ix.name.to_pascal_case()
            };

            // Generate fields (with annotations) inside the accounts struct recursively (sub accounts may exist)
            // Map from sub accounts struct name to the list of account names
            let mut map_sub_accounts: HashMap<String, Vec<String>> = HashMap::new();
            let mut accounts_to_keep: Vec<String> = Vec::new();
            let mut all_accounts: Vec<String> = Vec::new();
            let mut vec_accounts_ts: Vec<TokenStream> = Vec::new();

            let (_all_structs, _all_fields) = crate::generate_glam_account_fields(
                &ix.name.to_pascal_case(),
                &ix.accounts,
                ix_code_gen_configs.get(ix.name.as_str()),
                &mut vec_accounts_ts,
                &mut accounts_to_keep,
                &mut all_accounts,
                &mut map_sub_accounts,
                String::from("root"),
            );
            // println!("ix: {:?}", ix.name.as_str());
            // println!("map_sub_accounts: {:?}", map_sub_accounts);
            // println!("vec_accounts_ts: {:?}", vec_accounts_ts);

            ixs_sub_accounts.insert(ix.name.to_snake_case(), map_sub_accounts);
            // println!("all_accounts: {:?}", all_accounts);
            // println!("accounts_to_keep: {:?}", accounts_to_keep);

            let accounts_struct_name = {
                if accounts_structs_generated.contains(&accounts_struct_name_pascal_case) {
                    return quote! {};
                }
                accounts_structs_generated.push(accounts_struct_name_pascal_case.clone());

                format_ident!("{}{}", program_name, accounts_struct_name_pascal_case)
            };

            // Generate the remappings
            let mut glam_account_infos: Vec<AccuntInfo> = vec![];
            let mut index_map: Vec<i32> = vec![];
            let src_ix_name = ix.name.to_snake_case();
            let dst_ix_name = format!(
                "{}_{}",
                program_name.to_string().to_snake_case(),
                ix.name.to_snake_case()
            );
            let src_discriminator = compute_discriminator(&src_ix_name);
            let dst_discriminator = compute_discriminator(&dst_ix_name);

            let mut glam_ix_idx = 3;
            for idx in 0..all_accounts.len() {
                if !accounts_to_keep.contains(&all_accounts[idx]) {
                    index_map.push(-1);
                } else {
                    glam_ix_idx += 1;
                    index_map.push(glam_ix_idx);
                }
            }

            let glam_state_annotation = ix_code_gen_configs
                .get(ix.name.as_str())
                .map(|config| {
                    if config.mutable_state {
                        glam_account_infos.push(AccuntInfo {
                            name: "glam_state".to_string(),
                            index: 0,
                            writable: true,
                            signer: false,
                        });

                        quote! { #[account(mut)] }
                    } else {
                        glam_account_infos.push(AccuntInfo {
                            name: "glam_state".to_string(),
                            index: 0,
                            writable: false,
                            signer: false,
                        });

                        quote! {}
                    }
                })
                .unwrap_or(quote! {});

            let seeds =
                quote! { [crate::constants::SEED_VAULT.as_bytes(), glam_state.key().as_ref()] };
            let glam_vault_annotation =
                if let Some(config) = ix_code_gen_configs.get(ix.name.as_str()) {
                    if config.mutable_vault {
                        glam_account_infos.push(AccuntInfo {
                            name: "glam_vault".to_string(),
                            index: 1,
                            writable: true,
                            signer: false,
                        });

                        quote! { #[account(mut, seeds = #seeds, bump)] }
                    } else {
                        glam_account_infos.push(AccuntInfo {
                            name: "glam_vault".to_string(),
                            index: 1,
                            writable: false,
                            signer: false,
                        });

                        quote! { #[account(seeds = #seeds, bump)] }
                    }
                } else {
                    glam_account_infos.push(AccuntInfo {
                        name: "glam_vault".to_string(),
                        index: 1,
                        writable: false,
                        signer: false,
                    });

                    quote! { #[account(seeds = #seeds, bump)] }
                };

            let mut glam_accounts = TokenStream::new();
            glam_accounts.extend(quote! {
                #glam_state_annotation
                pub glam_state: Box<Account<'info, StateAccount>>,

                #glam_vault_annotation
                pub glam_vault: SystemAccount<'info>,

                #[account(mut)]
                pub glam_signer: Signer<'info>,

                // The same ix might allow multiple CPI programs (e.g., kamino mainnet staging & prod)
                // TODO: Support multiple CPI programs in one ix
                pub cpi_program: Program<'info, #program_name>,
            });

            glam_account_infos.push(AccuntInfo {
                name: "glam_signer".to_string(),
                index: 2,
                writable: true,
                signer: true,
            });
            glam_account_infos.push(AccuntInfo {
                name: "cpi_program".to_string(),
                index: 3,
                writable: false,
                signer: false,
            });

            // Create IxInfo
            // If ix is listed in input but not configured, it means we don't need to proxy it and
            // we don't need to generate remapping data
            if ixs_to_generate.contains(&ix.name.to_string())
                && ix_code_gen_configs.get(ix.name.as_str()).is_none()
            {
                ix_infos.push(IxInfo {
                    src_ix_name,
                    src_discriminator,
                    dst_ix_name: None,
                    dst_discriminator: None,
                    dynamic_accounts: None,
                    static_accounts: None,
                    index_map: None,
                });
            } else {
                ix_infos.push(IxInfo {
                    src_ix_name,
                    src_discriminator,
                    dst_ix_name: Some(dst_ix_name),
                    dst_discriminator: Some(dst_discriminator),
                    dynamic_accounts: Some(glam_account_infos),
                    static_accounts: Some(Vec::new()),
                    index_map: Some(index_map),
                });
            }

            quote! {
                #[derive(Accounts)]
                pub struct #accounts_struct_name<'info> {
                    #glam_accounts

                    #(#vec_accounts_ts),*
                }
            }
        });

    (quote! { #(#defs)* }, ix_infos, ixs_sub_accounts)
}

pub fn generate_ix_structs(ixs: &[IdlInstruction]) -> TokenStream {
    let defs = ixs.iter().map(|ix| {
        let accounts_name = format_ident!("{}", ix.name.to_pascal_case());

        let (all_structs, all_fields) =
            crate::generate_account_fields(&ix.name.to_pascal_case(), &ix.accounts);

        quote! {
            #all_structs

            #[derive(Accounts)]
            pub struct #accounts_name<'info> {

                #all_fields
            }
        }
    });
    quote! {
        #(#defs)*
    }
}

/// Generates all instruction handlers.
pub fn generate_ix_handlers(ixs: &[IdlInstruction]) -> TokenStream {
    let streams = ixs.iter().map(generate_ix_handler);
    quote! {
        #(#streams)*
    }
}

pub fn generate_glam_ix_handler(
    ix: &IdlInstruction,
    program_name: &Ident,
    ix_code_gen_config: &GlamIxCodeGenConfig,
    map_sub_accounts: &HashMap<String, Vec<String>>,
) -> TokenStream {
    let program_name_snake_case = format_ident!("{}", program_name.to_string().to_snake_case());
    let program_name_pascal_case = format_ident!("{}", program_name.to_string().to_pascal_case());

    let glam_ix_name = format_ident!("{}_{}", program_name_snake_case, ix.name.to_snake_case());
    let cpi_ix_name = format_ident!("{}", ix.name.to_snake_case());

    let cpi_ix_accounts_name = format_ident!("{}", ix.name.to_pascal_case());

    let glam_ix_accounts_name =
        if let Some(accounts_struct) = ix_code_gen_config.accounts_struct.clone() {
            format_ident!(
                "{}{}",
                program_name_pascal_case,
                accounts_struct.to_pascal_case()
            )
        } else {
            format_ident!("{}{}", program_name_pascal_case, ix.name.to_pascal_case())
        };

    let args = ix
        .args
        .iter()
        .map(|arg| {
            let name = format_ident!("{}", arg.name.to_snake_case());
            let type_name = crate::ty_to_rust_type(&arg.ty);
            let stream: proc_macro2::TokenStream = type_name.parse().unwrap();
            quote! {
                #name: #stream
            }
        })
        .collect::<Vec<_>>();

    let cpi_ix_args = ix
        .args
        .iter()
        .map(|arg| {
            let name = format_ident!("{}", arg.name.to_snake_case());
            quote! {
                #name
            }
        })
        .collect::<Vec<_>>();

    let mutable_state = ix_code_gen_config.mutable_state;
    let ctx_arg = if mutable_state {
        quote! { mut ctx }
    } else {
        quote! { ctx }
    };
    let pre_cpi = if let Some(pre_cpi) = ix_code_gen_config.pre_cpi.clone() {
        let func = format_ident!("{}", pre_cpi);
        quote! { crate::utils::pre_cpi::#func(&#ctx_arg, #(#cpi_ix_args),*)?; }
    } else {
        quote! {}
    };

    let vault_aliases = ix_code_gen_config.vault_aliases.clone().unwrap_or_default();

    let root_account_infos = map_sub_accounts
        .iter()
        .filter(|(k, _)| k.as_str() == "root")
        .map(|(_, v)| {
            let account_infos = v
                .iter()
                .map(|account| {
                    let name = format_ident!("{}", account.to_snake_case());
                    if vault_aliases.contains(&account.to_snake_case()) {
                        quote! {
                            #name: ctx.accounts.glam_vault.to_account_info()
                        }
                    } else {
                        quote! {
                            #name: ctx.accounts.#name.to_account_info()
                        }
                    }
                })
                .collect::<Vec<_>>();

            quote! {
               #(#account_infos,)*
            }
        })
        .collect::<Vec<_>>();

    let sub_account_infos = map_sub_accounts
        .iter()
        .filter(|(k, _)| k.as_str() != "root")
        .map(|(k, v)| {
            let sub_account_infos = v
                .iter()
                .map(|account| {
                    let name = format_ident!("{}", account.to_snake_case());
                    if vault_aliases.contains(&account.to_snake_case()) {
                        quote! {
                            #name: ctx.accounts.glam_vault.to_account_info()
                        }
                    } else {
                        quote! {
                            #name: ctx.accounts.#name.to_account_info()
                        }
                    }
                })
                .collect::<Vec<_>>();
            let sub_account_name = format_ident!("{}", k);
            let sub_account_struct_name = format_ident!("{}", k.to_snake_case().to_pascal_case());
            quote! {
                #sub_account_name:  #program_name_snake_case::cpi::accounts::#sub_account_struct_name {
                    #(#sub_account_infos),*
                }
            }
        })
        .collect::<Vec<_>>();

    let access_control_permission = if let Some(permission) = &ix_code_gen_config.permission {
        let permission = format_ident!("{}", permission);
        quote! {
            #[access_control(acl::check_access(&ctx.accounts.glam_state, &ctx.accounts.glam_signer.key, Permission::#permission))]
        }
    } else {
        quote! {}
    };

    let access_control_integration = if let Some(integration) = &ix_code_gen_config.integration {
        let integration = format_ident!("{}", integration);
        quote! {
            #[access_control(acl::check_integration(&ctx.accounts.glam_state, Integration::#integration))]
        }
    } else {
        quote! {}
    };

    let (lt0, lt1, lt2, lt3) = if ix_code_gen_config.with_remaining_accounts {
        (
            quote! { <'c: 'info, 'info> },
            quote! { '_, '_, 'c, 'info, },
            quote! { <'info> },
            quote! { .with_remaining_accounts(ctx.remaining_accounts.to_vec())},
        )
    } else {
        (quote! {}, quote! {}, quote! {}, quote! {})
    };

    if ix_code_gen_config.signed_by_vault {
        quote! {
            #access_control_permission
            #access_control_integration
            #[glam_macros::glam_vault_signer_seeds]
            pub fn #glam_ix_name #lt0(
                #ctx_arg: Context<#lt1 #glam_ix_accounts_name #lt2>,
                #(#args),*
            ) -> Result<()> {
                #pre_cpi

                #program_name_snake_case::cpi::#cpi_ix_name(CpiContext::new_with_signer(
                    ctx.accounts.cpi_program.to_account_info(),
                    #program_name_snake_case::cpi::accounts::#cpi_ix_accounts_name {
                        #(#sub_account_infos,)*
                        #(#root_account_infos)*
                    },
                    glam_vault_signer_seeds
                )#lt3,#(#cpi_ix_args),*)
            }
        }
    } else {
        quote! {
            #access_control_permission
            #access_control_integration
            pub fn #glam_ix_name(
                #ctx_arg: Context<#glam_ix_accounts_name>,
                #(#args),*
            ) -> Result<()> {
                #pre_cpi

                #program_name_snake_case::cpi::#cpi_ix_name(CpiContext::new(
                    ctx.accounts.cpi_program.to_account_info(),
                    #program_name_snake_case::cpi::accounts::#cpi_ix_accounts_name {
                        #(#sub_account_infos,)*
                        #(#root_account_infos)*
                    },
                ),#(#cpi_ix_args),*)
            }
        }
    }
}

pub fn generate_glam_ix_handlers(
    ixs: &[IdlInstruction],
    program_name: &Ident,
    ixs_to_generate: &[String],
    ix_code_gen_configs: &HashMap<String, GlamIxCodeGenConfig>,
    ixs_sub_accounts: &HashMap<String, HashMap<String, Vec<String>>>,
) -> TokenStream {
    let streams = ixs
        .iter()
        .filter(|ix| ixs_to_generate.is_empty() || ixs_to_generate.contains(&ix.name.to_string()))
        .map(|ix| {
            let ix_code_gen_config = ix_code_gen_configs
                .get(ix.name.as_str())
                .cloned()
                .unwrap_or_default();

            let map_sub_accounts = ixs_sub_accounts
                .get(ix.name.to_snake_case().as_str())
                .cloned()
                .unwrap_or_default();

            generate_glam_ix_handler(ix, program_name, &ix_code_gen_config, &map_sub_accounts)
        });
    quote! {
        #(#streams)*
    }
}
