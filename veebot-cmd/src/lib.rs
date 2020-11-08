use proc_macro::TokenStream;
use quote::quote;

/// Small proc-macro-attribute to reduce the boilerplate for creating
/// bot commands with the `serenity` framework.
/// This wraps the standard `serenity::command` proc macro
/// allowing us to use our own application specific `veebot::Error`,
/// plus the generated code also send the error information to the
/// chat the command came from using its `create_msg` method.
#[proc_macro_attribute]
pub fn veebot_cmd(attr: TokenStream, item: TokenStream) -> TokenStream {
    assert!(
        attr.is_empty(),
        "veebot cmd must be used as a bare attribute without any arguments"
    );

    let mut fn_item = syn::parse_macro_input!(item as syn::ItemFn);
    let vis = &fn_item.vis;
    let cmd_name = &fn_item.sig.ident;
    let (fn_args, fn_arg_idents): (Vec<_>, Vec<_>) = fn_item
        .sig
        .inputs
        .iter()
        .cloned()
        .enumerate()
        .map(|(i, mut fn_arg)| {
            let pat_type = match &mut fn_arg {
                syn::FnArg::Receiver(_) => unreachable!(),
                syn::FnArg::Typed(it) => it,
            };
            let ident = quote::format_ident!("arg_{}", i);
            *pat_type.pat = syn::parse_quote!(#ident);
            (fn_arg, ident)
        })
        .unzip();

    let attrs = std::mem::take(&mut fn_item.attrs);

    let result = quote! {
        #[::serenity::framework::standard::macros::command]
        #(#attrs)*
        #vis async fn #cmd_name(#(#fn_args),*) -> ::serenity::framework::standard::CommandResult {
            if let Err(err) = #cmd_name(#(#fn_arg_idents),*).await {
                arg_1.channel_id
                    .send_message(arg_0, |it| err.create_msg(it))
                    .await
                    .unwrap();
            }

            return Ok(());

            #fn_item
        }
    };

    result.into()
}
