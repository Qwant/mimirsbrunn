use crate::errors::TaggerError;
use crate::tagger::Tagger;
use once_cell::sync::Lazy;
use postal::{Context, InitOptions, ParseAddressOptions};

/// This is needed to use libpostal C bindings on the tokio runtime
unsafe impl Sync for AddressTagger {}
unsafe impl Send for AddressTagger {}

pub static ADDRESS_TAGGER: Lazy<AddressTagger> = Lazy::new(|| {
    let mut ctx = Context::new();
    let _ = ctx.init(InitOptions {
        expand_address: false,
        parse_address: true,
    });

    AddressTagger { inner: ctx }
});

/// A tagger to identify either address or a street name
/// ```rust, ignore
/// use tagger::{AddressTag, Tagger};
/// use crate::tagger::ADDRESS_TAGGER;
///
/// assert_eq!(
///      ADDRESS_TAGGER.tag("156BIS Route de Dijon Brazey-en-Plaine", Some(0)).unwrap(),
///      Some(AddressTag::Address)
///  );
///  assert_eq!(
///      ADDRESS_TAGGER.tag("Route de Dijon Brazey-en-Plaine", Some(0)).unwrap(),
///      Some(AddressTag::Street)
///  );
/// ```
pub struct AddressTagger {
    inner: Context,
}

#[derive(Debug, Eq, PartialEq)]
pub enum AddressTag {
    Street,
    Address,
}

impl Tagger for AddressTagger {
    type Output = Result<Option<AddressTag>, TaggerError>;
    fn tag(&self, input: &str, _: Option<u32>) -> Self::Output {
        let mut tag = None;

        self.inner
            .parse_address(input, &mut ParseAddressOptions::new())
            .map_err(TaggerError::from)
            .map(|comps| {
                for c in comps {
                    if c.label == "house_number" {
                        tag = Some(AddressTag::Address)
                    }

                    if c.label == "road" && tag.is_none() {
                        tag = Some(AddressTag::Street)
                    }
                }

                tag
            })
    }
}

#[cfg(test)]
mod test {
    use crate::tagger::address::{AddressTag, ADDRESS_TAGGER};
    use crate::tagger::Tagger;

    #[test]
    fn address_tagger_works() {
        assert_eq!(
            ADDRESS_TAGGER
                .tag("156BIS Route de Dijon Brazey-en-Plaine", Some(0))
                .unwrap(),
            Some(AddressTag::Address)
        );

        assert_eq!(
            ADDRESS_TAGGER
                .tag("156ter Route de Dijon Brazey-en-Plaine", Some(0))
                .unwrap(),
            Some(AddressTag::Address)
        );

        assert_eq!(
            ADDRESS_TAGGER
                .tag("Route de Dijon Brazey-en-Plaine", Some(0))
                .unwrap(),
            Some(AddressTag::Street)
        );
        assert!(ADDRESS_TAGGER.tag("toto", Some(0)).unwrap().is_none());

        assert!(ADDRESS_TAGGER
            .tag("mouse au chocolat", Some(0))
            .unwrap()
            .is_none());

        assert_eq!(
            ADDRESS_TAGGER.tag("Place de l'étoile", Some(0)).unwrap(),
            Some(AddressTag::Street)
        );
    }
}
