//! FI-077 — no message causes a panic, at the *dispatch* level.
//!
//! `message.rs`'s `Message::decode` already never panics on malformed
//! bytes (confirmed via `catch_unwind` fuzzing). This module extends that
//! guarantee one layer up: to whatever a handler actually *does* with a
//! successfully-decoded message. A handler is arbitrary caller code, and
//! arbitrary code can panic (an unwrap on unexpected handler-internal
//! state, an index out of bounds, an integer overflow in debug builds,
//! ...) — [`dispatch`] catches that and turns it into an `Err`, so one
//! misbehaving handler can't take down whatever's driving the dispatch
//! loop.

use std::panic::AssertUnwindSafe;

use crate::message::SignedMessage;

pub trait MessageHandler {
    type Response;
    type Error: std::fmt::Display + std::fmt::Debug;

    fn handle(&self, msg: &SignedMessage) -> Result<Self::Response, Self::Error>;
}

#[derive(Debug, thiserror::Error)]
pub enum DispatchError<E: std::fmt::Display + std::fmt::Debug> {
    #[error("handler returned an error: {0}")]
    Handler(E),
    #[error("handler panicked: {0}")]
    Panic(String),
}

/// Runs `handler.handle(msg)` inside `catch_unwind`. A panic is logged
/// (`tracing::error!`) and converted to `Err(DispatchError::Panic)` instead
/// of propagating and aborting whatever called `dispatch`.
pub fn dispatch<H: MessageHandler>(handler: &H, msg: &SignedMessage) -> Result<H::Response, DispatchError<H::Error>> {
    match std::panic::catch_unwind(AssertUnwindSafe(|| handler.handle(msg))) {
        Ok(Ok(response)) => Ok(response),
        Ok(Err(e)) => Err(DispatchError::Handler(e)),
        Err(payload) => {
            let message = panic_payload_message(&payload);
            tracing::error!(panic = %message, "message handler panicked");
            Err(DispatchError::Panic(message))
        }
    }
}

fn panic_payload_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_signed_message() -> SignedMessage {
        use crate::message::{sign_message, Message};
        use arkhe_crypto_pqc::generate_hybrid_keypair;

        let kp = generate_hybrid_keypair().unwrap();
        let msg = Message { sender_id: "vm-1".to_string(), nonce: 1, payload: b"hello".to_vec() };
        sign_message(&kp, &msg).unwrap()
    }

    struct EchoHandler;
    impl MessageHandler for EchoHandler {
        type Response = Vec<u8>;
        type Error = std::convert::Infallible;
        fn handle(&self, msg: &SignedMessage) -> Result<Vec<u8>, Self::Error> {
            Ok(msg.message.payload.clone())
        }
    }

    struct FailingHandler;
    #[derive(Debug, thiserror::Error)]
    #[error("handler deliberately failed")]
    struct FailingError;
    impl MessageHandler for FailingHandler {
        type Response = ();
        type Error = FailingError;
        fn handle(&self, _msg: &SignedMessage) -> Result<(), FailingError> {
            Err(FailingError)
        }
    }

    struct PanickingHandler;
    impl MessageHandler for PanickingHandler {
        type Response = ();
        type Error = std::convert::Infallible;
        fn handle(&self, _msg: &SignedMessage) -> Result<(), Self::Error> {
            panic!("handler blew up");
        }
    }

    #[test]
    fn successful_handler_returns_ok() {
        let result = dispatch(&EchoHandler, &sample_signed_message());
        assert_eq!(result.unwrap(), b"hello".to_vec());
    }

    #[test]
    fn handler_error_is_propagated_as_dispatch_error() {
        let result = dispatch(&FailingHandler, &sample_signed_message());
        assert!(matches!(result, Err(DispatchError::Handler(FailingError))));
    }

    #[test]
    fn panicking_handler_does_not_abort_dispatch() {
        // The real assertion: this test itself does not crash/abort even
        // though the handler panics internally — catch_unwind contained it.
        let result = std::panic::catch_unwind(|| dispatch(&PanickingHandler, &sample_signed_message()));
        assert!(result.is_ok(), "dispatch() itself must not panic even when the handler does");
        assert!(matches!(result.unwrap(), Err(DispatchError::Panic(_))));
    }
}
