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

    let fn_item = syn::parse_macro_input!(item as syn::ItemFn);
    let vis = &fn_item.vis;
    let cmd_name = &fn_item.sig.ident;

    let result = quote! {
        #[::serenity::framework::standard::macros::command]
        #vis async fn #cmd_name(
            ctx: &::serenity::client::Context,
            msg: &::serenity::model::channel::Message,
            args: ::serenity::framework::standard::Args,
        ) -> ::serenity::framework::standard::CommandResult {
            if let Err(err) = #cmd_name(ctx, msg, args).await {
                msg.channel_id
                    .send_message(ctx, |it| err.create_msg(it))
                    .await
                    .unwrap();
            }

            return Ok(());

            #fn_item
        }
    };

    result.into()
}
