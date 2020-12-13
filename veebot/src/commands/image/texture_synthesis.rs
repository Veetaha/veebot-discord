use std::iter;

use crate::di::{self, DiExt};
use crate::util::ReqwestBuilderExt;
use itertools::Itertools;
use serenity::{
    client::Context,
    framework::standard::Args,
    model::{channel::Message, id::ChannelId},
};
use texture_synthesis as ts;
use tracing::debug;
use ts::image::GenericImageView;
use url::Url;
use veebot_cmd::veebot_cmd;

#[veebot_cmd]
#[aliases("st", "ts")]
pub(crate) async fn synthesize_texture(
    ctx: &Context,
    msg: &Message,
    args: Args,
) -> crate::Result<()> {
    let images = get_images(ctx, args).await?;
    let image = run_ts_session(|session| session.add_examples(images).seed(rand::random())).await?;

    send_generated_image(ctx, image, msg.channel_id).await?;
    Ok(())
}

#[veebot_cmd]
#[aliases("as")]
pub(crate) async fn apply_style(ctx: &Context, msg: &Message, mut args: Args) -> crate::Result<()> {
    let guide_alpha = args.single::<f32>().unwrap_or_else(|_| {
        args.rewind();
        1.0
    });

    let images = get_images(ctx, args).await?;
    let actual = images.len();

    let (style_guide, texture) = images.into_iter().collect_tuple().ok_or_else(|| {
        crate::err!(InvalidNumberOfArguments {
            expected: 2,
            actual
        })
    })?;

    let dims = ts::Dims::new(style_guide.width(), style_guide.height());

    let image = run_ts_session(move |session| {
        session
            .add_example(texture)
            .load_target_guide(style_guide)
            .guide_alpha(guide_alpha)
            .output_size(dims)
    })
    .await?;

    send_generated_image(ctx, image, msg.channel_id).await?;

    Ok(())
}

async fn run_ts_session(
    configure_session: impl 'static + Send + FnOnce(ts::SessionBuilder<'_>) -> ts::SessionBuilder<'_>,
) -> crate::Result<ts::GeneratedImage> {
    tokio::task::spawn_blocking(move || {
        Ok(configure_session(ts::Session::builder())
            .build()?
            .run(Some(Box::new(|it: ts::ProgressUpdate<'_>| {
                debug!(
                    "Processing {:.1}%",
                    100.0 * it.total.current as f64 / it.total.total as f64
                )
            }))))
    })
    .await?
}

async fn send_generated_image(
    ctx: &Context,
    image: ts::GeneratedImage,
    channel_id: ChannelId,
) -> crate::Result<()> {
    let bytes = generated_image_into_jpeg_bytes(image)?;
    let files = iter::once((bytes.as_slice(), "generated.jpg"));

    channel_id.send_files(&ctx, files, |it| it).await?;
    Ok(())
}

async fn get_images(ctx: &Context, mut args: Args) -> crate::Result<Vec<ts::image::DynamicImage>> {
    let http_client = ctx.data.expect_dep::<di::HttpClientToken>().await;
    let futs = args.iter::<Url>().map(|image_url| async {
        let bytes = http_client.get(image_url?).read_bytes().await?;
        Ok(ts::image::load_from_memory(&bytes).map_err(ts::Error::Image)?)
    });

    futures::future::try_join_all(futs).await
}

fn generated_image_into_jpeg_bytes(image: ts::GeneratedImage) -> Result<Vec<u8>, ts::Error> {
    let mut bytes = Vec::new();
    image.write(&mut bytes, ts::image::ImageOutputFormat::Jpeg(100))?;
    Ok(bytes)
}
