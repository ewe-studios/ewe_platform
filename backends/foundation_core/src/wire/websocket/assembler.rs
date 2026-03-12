//! Message assembler for fragmented WebSocket messages (RFC 6455 Section 4.5).
//!
//! WHY: WebSocket messages can be split across multiple frames (fragmentation).
//! The assembler collects fragments and reassembles them into complete messages.
//!
//! WHAT: `MessageAssembler` accumulates frame fragments and validates the sequence.
//! Handles interleaved control frames during fragmentation per RFC 6455 Section 4.6.
//!
//! HOW: Tracks the initial frame opcode (Text/Binary), accumulates payload data,
//! and validates continuation frame sequence. Performs UTF-8 validation for text
//! messages and enforces maximum message size.
//!
//! # RFC 6455 Compliance
//!
//! - Section 4.5: Fragmentation rules
//! - Section 4.6: Control frames can be interleaved
//! - Section 4.8: Maximum message size enforcement
//! - Section 5.6: UTF-8 validation for text messages

use super::error::WebSocketError;
use super::frame::{Opcode, WebSocketFrame};
use super::message::WebSocketMessage;

/// Default maximum message size (64 MiB).
///
/// WHY: Prevents memory exhaustion from unbounded message accumulation.
/// WHAT: Applications can configure this limit based on their needs.
const DEFAULT_MAX_MESSAGE_SIZE: usize = 64 * 1024 * 1024;

/// Assembly state for a fragmented message.
///
/// WHY: Need to track partial message state between frame arrivals.
/// WHAT: Holds the initial opcode, accumulated payload, UTF-8 validation state, and size limit.
#[derive(Debug, Clone)]
struct FragmentState {
    /// Initial frame opcode (must be Text or Binary).
    opcode: Opcode,
    /// Accumulated payload data from all fragments.
    payload: Vec<u8>,
    /// For text messages, tracks if accumulated data is valid UTF-8 so far.
    /// Stores the index of the last incomplete UTF-8 sequence start.
    utf8_valid_up_to: usize,
    /// Maximum allowed message size (copied from parent assembler).
    max_message_size: usize,
}

/// Assembles fragmented WebSocket messages.
///
/// WHY: RFC 6455 allows messages to be split across multiple frames.
/// The assembler collects fragments and reassembles them into complete messages.
///
/// WHAT: Tracks fragment state, validates sequence, and enforces size limits.
///
/// HOW: On each fragment, validates the sequence, accumulates payload, and
/// checks size limits. When FIN=1, returns the complete message.
///
/// # Examples
///
/// ```no_run
/// use foundation_core::wire::websocket::MessageAssembler;
/// use foundation_core::wire::websocket::WebSocketFrame;
///
/// let mut assembler = MessageAssembler::new(1024 * 1024); // 1 MiB limit
/// // Process frames as they arrive...
/// ```
#[derive(Clone)]
pub struct MessageAssembler {
    /// Current fragmentation state, if assembling a message.
    state: Option<FragmentState>,
    /// Maximum allowed message size in bytes.
    max_message_size: usize,
}

impl MessageAssembler {
    /// Create a new message assembler with a maximum message size.
    ///
    /// # Arguments
    ///
    /// * `max_message_size` - Maximum allowed message size in bytes
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::websocket::MessageAssembler;
    ///
    /// let assembler = MessageAssembler::new(1024 * 1024); // 1 MiB limit
    /// ```
    #[must_use]
    pub fn new(max_message_size: usize) -> Self {
        Self {
            state: None,
            max_message_size,
        }
    }

    /// Create a new message assembler with default max size (64 MiB).
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::websocket::MessageAssembler;
    ///
    /// let assembler = MessageAssembler::default();
    /// ```
    #[must_use]
    pub fn with_default_limit() -> Self {
        Self {
            state: None,
            max_message_size: DEFAULT_MAX_MESSAGE_SIZE,
        }
    }

    /// Process an incoming frame.
    ///
    /// WHY: Each frame needs to be processed and potentially assembled into a message.
    ///
    /// WHAT: Returns `Ok(Some(message))` when a complete message is assembled,
    /// `Ok(None)` when more fragments are needed, or `Err` on protocol violation.
    ///
    /// # Arguments
    ///
    /// * `frame` - The decoded WebSocket frame
    ///
    /// # Returns
    ///
    /// * `Ok(Some(WebSocketMessage))` - Complete message assembled
    /// * `Ok(None)` - More fragments needed, or control frame handled
    /// * `Err(WebSocketError)` - Protocol violation or size limit exceeded
    ///
    /// # Errors
    ///
    /// Returns `WebSocketError::ProtocolError` for:
    /// - Unexpected continuation frame (no fragmentation in progress)
    /// - Fragmented control frame (not allowed)
    /// - Invalid fragment sequence (Text/Binary after Continuation started)
    /// - Message size limit exceeded
    /// - Invalid UTF-8 in text message
    ///
    /// # Panics
    ///
    /// Never panics.
    pub fn process_frame(
        &mut self,
        frame: WebSocketFrame,
    ) -> Result<Option<WebSocketMessage>, WebSocketError> {
        // Validate control frames are not fragmented
        if frame.opcode.is_control() {
            if !frame.fin {
                return Err(WebSocketError::ProtocolError(
                    "control frames must not be fragmented".to_string(),
                ));
            }
            // Control frames are always complete - return immediately
            return Ok(Some(frame.to_message()?));
        }

        // Handle data frames (Text, Binary, Continuation)
        match frame.opcode {
            Opcode::Continuation => {
                // Continuation frame - must be part of an existing fragmentation
                let state = self.state.as_mut().ok_or_else(|| {
                    WebSocketError::ProtocolError(
                        "unexpected Continuation frame (no message in progress)".to_string(),
                    )
                })?;

                // Check size limit before adding payload
                let new_size = state.payload.len() + frame.payload.len();
                if new_size > state.max_message_size {
                    return Err(WebSocketError::ProtocolError(format!(
                        "message size limit exceeded: {} bytes (max {})",
                        new_size, state.max_message_size
                    )));
                }

                // Accumulate payload
                state.payload.extend_from_slice(&frame.payload);

                // Update UTF-8 validation state for text messages
                if state.opcode == Opcode::Text {
                    state.utf8_valid_up_to =
                        validate_utf8_incremental(&state.payload, state.utf8_valid_up_to)?;
                }

                // Check if this is the final fragment
                if frame.fin {
                    // Fragmentation complete - return assembled message
                    let state = self.state.take().unwrap();
                    let message = match state.opcode {
                        Opcode::Text => {
                            // UTF-8 already validated incrementally
                            let text = String::from_utf8(state.payload)
                                .map_err(WebSocketError::InvalidUtf8)?;
                            WebSocketMessage::Text(text)
                        }
                        Opcode::Binary => WebSocketMessage::Binary(state.payload),
                        _ => unreachable!("only Text or Binary can start fragmentation"),
                    };
                    Ok(Some(message))
                } else {
                    // More fragments coming
                    Ok(None)
                }
            }

            Opcode::Text | Opcode::Binary => {
                // Starting a new message - check if one is already in progress
                if self.state.is_some() {
                    return Err(WebSocketError::ProtocolError(
                        "new message started before completing previous fragmented message"
                            .to_string(),
                    ));
                }

                // Check size limit
                if frame.payload.len() > self.max_message_size {
                    return Err(WebSocketError::ProtocolError(format!(
                        "message size limit exceeded: {} bytes (max {})",
                        frame.payload.len(),
                        self.max_message_size
                    )));
                }

                if frame.fin {
                    // Complete unfragmented message - return immediately
                    // (no need to update state)
                    Ok(Some(frame.to_message()?))
                } else {
                    // First fragment of a multi-fragment message
                    let utf8_valid_up_to = if frame.opcode == Opcode::Text {
                        validate_utf8_incremental(&frame.payload, 0)?
                    } else {
                        0
                    };

                    self.state = Some(FragmentState {
                        opcode: frame.opcode,
                        payload: frame.payload,
                        utf8_valid_up_to,
                        max_message_size: self.max_message_size,
                    });
                    Ok(None)
                }
            }

            Opcode::Close | Opcode::Ping | Opcode::Pong => {
                // Should have been handled by control frame check above
                unreachable!("control frames handled earlier")
            }
        }
    }

    /// Check if currently assembling a fragmented message.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::websocket::MessageAssembler;
    ///
    /// let mut assembler = MessageAssembler::default();
    /// assert!(!assembler.is_assembling());
    /// ```
    #[must_use]
    pub fn is_assembling(&self) -> bool {
        self.state.is_some()
    }

    /// Get the current accumulated payload size (for fragmented messages).
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::websocket::MessageAssembler;
    ///
    /// let mut assembler = MessageAssembler::default();
    /// assert_eq!(assembler.accumulated_size(), 0);
    /// ```
    #[must_use]
    pub fn accumulated_size(&self) -> usize {
        self.state.as_ref().map(|s| s.payload.len()).unwrap_or(0)
    }

    /// Reset the assembler state (abort any in-progress fragmentation).
    ///
    /// WHY: Applications may want to abort a partial message on timeout or error.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::websocket::MessageAssembler;
    ///
    /// let mut assembler = MessageAssembler::default();
    /// assembler.reset();
    /// ```
    pub fn reset(&mut self) {
        self.state = None;
    }
}

impl Default for MessageAssembler {
    fn default() -> Self {
        Self::with_default_limit()
    }
}

/// Validate UTF-8 incrementally, returning the index up to which data is valid.
///
/// WHY: For fragmented text messages, we need to validate UTF-8 as data arrives
/// without re-validating already-checked bytes.
///
/// WHAT: Returns `Ok(index)` where all bytes before `index` are valid UTF-8,
/// or `Err` if invalid UTF-8 is detected.
///
/// HOW: Uses `std::str::from_utf8` on the suffix starting from `valid_up_to`.
/// If the suffix ends with an incomplete UTF-8 sequence, returns the index
/// where the incomplete sequence starts.
///
/// # Arguments
///
/// * `data` - The accumulated payload data
/// * `valid_up_to` - Index up to which data was previously validated
///
/// # Returns
///
/// * `Ok(usize)` - Index up to which data is now validated
/// * `Err(WebSocketError::InvalidUtf8)` - Invalid UTF-8 detected
fn validate_utf8_incremental(data: &[u8], valid_up_to: usize) -> Result<usize, WebSocketError> {
    if valid_up_to >= data.len() {
        // All data already validated
        return Ok(data.len());
    }

    // Validate the suffix
    let suffix = &data[valid_up_to..];
    match std::str::from_utf8(suffix) {
        Ok(_) => {
            // Entire suffix is valid UTF-8
            Ok(data.len())
        }
        Err(e) => {
            // Check if error is due to invalid bytes (not incomplete sequence)
            if e.error_len().is_some() {
                // Invalid bytes in middle - create a FromUtf8Error by trying to convert invalid bytes
                return Err(WebSocketError::InvalidUtf8(
                    String::from_utf8(suffix.to_vec()).unwrap_err(),
                ));
            }
            // Incomplete sequence at end - return valid length
            // `e.valid_up_to()` gives index relative to suffix
            Ok(valid_up_to + e.valid_up_to())
        }
    }
}
