mod texture_synthesis;

pub(crate) use self::texture_synthesis::*;
use serenity::framework::standard::macros::group;

#[group]
#[commands(synthesize_texture, apply_style)]
pub(crate) struct Image;
