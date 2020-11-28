use serenity::{client::Context, framework::standard::{Args, macros::group}, model::channel::Message};
use texture_synthesis as ts;
use veebot_cmd::veebot_cmd;

#[group]
#[commands(synthesize_texture)]
pub(crate) struct Image;

#[veebot_cmd]
#[aliases("st")]
pub(crate) async fn synthesize_texture(ctx: &Context, msg: &Message, mut args: Args) -> crate::Result<()> {
    let image_url = args.single::<url::Url>()?;

    let http_client = crate::util::create_http_client();

    let image = http_client
        .get(image_url)
        .send()
        .await
        .expect("TODO: handle")
        .bytes()
        .await
        .expect("TODO: handle");

    let image = tokio::task::spawn_blocking(
        move || {
            ts::Session::builder()
                .add_example(ts::ImageSource::Memory(&image))
                .seed(rand::random())
                .build()
                .expect("TODO: handle")
                .run(Some(Box::new(|it: ts::ProgressUpdate<'_>| {
                    dbg!(it.total.current, it.total.total, it.stage.current, it.stage.total);
                })))
        })
        .await
        .unwrap();

    let mut bytes = Vec::new();
    image.write(&mut bytes, ts::image::ImageOutputFormat::Jpeg(100)).expect("TODO: handle");

    let files = vec![(bytes.as_slice(), "generated.jpg")];

    msg
        .channel_id
        .send_files(&ctx, files, |it| it)
        .await?;

    dbg!();

    Ok(())
}
