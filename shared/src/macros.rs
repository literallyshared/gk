#[macro_export]
macro_rules! message_definitions {
    (
        $vis:vis enum $message_ident:ident {
            opcode => $opcode_ident:ident;
            $(
                $variant:ident ( $payload:ty ) = $opcode_value:expr;
            )+ $(,)?
        }
    ) => {
        #[derive(Clone, Copy)]
        $vis enum $opcode_ident {
            $(
                $variant = $opcode_value,
            )+
        }

        impl $crate::frame::Opcode for $opcode_ident {
            fn from_raw(raw: u16) -> Result<Self, u16> {
                match raw {
                    $(
                        $opcode_value => Ok(Self::$variant),
                    )+
                    other => Err(other),
                }
            }

            fn into_raw(self) -> u16 {
                self as u16
            }
        }

        #[derive(Debug)]
        $vis enum $message_ident {
            $(
                $variant($payload),
            )+
        }

        impl $message_ident {
            pub fn deserialize(
                opcode: $opcode_ident,
                bytes: &[u8],
            ) -> Result<Self, ::bincode::error::DecodeError> {
                let config = ::bincode::config::legacy();
                match opcode {
                    $(
                        $opcode_ident::$variant => Ok(Self::$variant(
                            ::bincode::serde::decode_from_slice(bytes, config)?.0,
                        )),
                    )+
                }
            }

            pub fn serialize(
                &self,
            ) -> Result<$crate::frame::MessageFrame<$opcode_ident>, ::bincode::error::EncodeError>
            {
                let config = ::bincode::config::legacy();
                match self {
                    $(
                        Self::$variant(inner) => Ok($crate::frame::MessageFrame::new(
                            $opcode_ident::$variant,
                            ::bincode::serde::encode_to_vec(inner, config)?,
                        )),
                    )+
                }
            }
        }
    };
}
