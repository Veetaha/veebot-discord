use serenity::{client::Context, framework::standard::Args, model::channel::Message};
use veebot_cmd::veebot_cmd;

#[veebot_cmd]
#[aliases("gg")]
pub(crate) async fn gen_gif(ctx: &Context, msg: &Message, args: Args) -> crate::Result<()> {
    // private async createWelcomeMemberImgStream(newMember: ds.GuildMember | ds.PartialGuildMember) {
    //     const welcomeText = `Hi, ${newMember.displayName}!`;
    //     const fontFace = 'Consolas';
    //     const fontSize = this.canvasUtils.getFontSizeToFit(welcomeText, fontFace, 900);

    //     return im(GifId.MemberWelcomeBg)
    //         .coalesce()
    //         .quality(100)
    //         .stroke("#000")
    //         .strokeWidth(3)
    //         .fill("#6395ea")
    //         .font(`${fontFace}.ttf`, fontSize)
    //         .dither(false)
    //         .colors(128)
    //         .drawText(0, 0, welcomeText, 'South')
    //         .stream('gif');
    // }
    Ok(())
}
