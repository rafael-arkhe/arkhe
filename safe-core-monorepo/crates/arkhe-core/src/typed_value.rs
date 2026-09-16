//! Valor tipado — a carga de uma decisão.
//!
//! O `arkhe-core` não tinha nenhum tipo que representasse o **resultado** de
//! uma decisão: o [`ArkheError`](crate::ArkheError) cobre a falha, e
//! [`SafetyVerdict`](crate::SafetyVerdict) cobre o permitido/rejeitado da
//! verificação de segurança, mas nenhum deles carrega um valor de domínio
//! arbitrário. Este módulo fecha essa lacuna com um enum fechado de valores.
//!
//! O nome vem de "typed value" no sentido estrito: o valor viaja com a sua
//! forma (booleano, inteiro, vírgula flutuante, texto, bytes, ou sequência),
//! pelo que quem o consome não tem de reinterpretar uma `String` nem fazer
//! *downcast*. É o tipo de saída que a decisão calibrada de
//! `arkhe-verify::calibration` transporta.

use serde::{Deserialize, Serialize};

/// Valor tipado — output de uma decisão calibrada.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TypedValue {
    /// Booleano.
    Bool(bool),
    /// Inteiro com sinal.
    Int(i64),
    /// Vírgula flutuante.
    Float(f64),
    /// Texto.
    String(String),
    /// Bytes opacos.
    Bytes(Vec<u8>),
    /// Sequência de valores tipados.
    Array(Vec<TypedValue>),
}
