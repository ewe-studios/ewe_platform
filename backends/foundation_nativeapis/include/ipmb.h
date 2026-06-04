/**
 * ipmb.h — C bindings for the foundation_nativeapis IPC message bus.
 *
 * Provides a bus-based IPC where endpoints join a named bus, send typed
 * BytesMessage payloads, and receive messages via opaque handles.
 *
 * Usage:
 *   ipmb_Sender*   sender   = NULL;
 *   ipmb_Receiver* receiver = NULL;
 *   ipmb_Options opts = {
 *       .identifier = "com.ewe.mybus",
 *       .label = "my-endpoint",
 *       .token = NULL,
 *       .controller_affinity = 1,
 *   };
 *   if (ipmb_join(opts, 5000, &sender, &receiver) != 0) { ... }
 *
 *   // Send
 *   const char* data = "hello";
 *   ipmb_send(sender, 0, (const uint8_t*)data, 5);
 *
 *   // Receive
 *   ipmb_Message* msg = NULL;
 *   if (ipmb_recv(receiver, &msg, 2000) == 0) {
 *       uint32_t len = 0;
 *       const uint8_t* bytes = ipmb_message_data(msg, &len);
 *       uint16_t fmt = ipmb_message_format(msg);
 *       // ... use bytes[0..len] ...
 *       ipmb_message_free(msg);
 *   }
 *
 *   ipmb_sender_free(sender);
 *   ipmb_receiver_free(receiver);
 */

#ifndef IPMB_H
#define IPMB_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Opaque handles */
typedef struct ipmb_Sender   ipmb_Sender;
typedef struct ipmb_Receiver ipmb_Receiver;
typedef struct ipmb_Message  ipmb_Message;

/* Options for joining a bus */
typedef struct {
    const char* identifier;         /* bus name, e.g. "com.ewe.watchers" */
    const char* label;              /* endpoint label for routing */
    const char* token;              /* auth token (NULL for none) */
    int         controller_affinity; /* non-zero to become controller if none exists */
} ipmb_Options;

/**
 * Join a message bus. On success, writes sender and receiver handles.
 * @param timeout_ms  Timeout in ms (0 = no timeout).
 * @return 0 on success, -1 on error.
 */
int32_t ipmb_join(ipmb_Options options, uint32_t timeout_ms,
                  ipmb_Sender** out_sender, ipmb_Receiver** out_receiver);

/**
 * Send a BytesMessage on the bus (broadcast to all endpoints).
 * @param format  Application-defined format tag.
 * @param data    Pointer to payload bytes (may be NULL if data_len is 0).
 * @param data_len Length of payload in bytes.
 * @return 0 on success, -1 on error.
 */
int32_t ipmb_send(ipmb_Sender* sender, uint16_t format,
                  const uint8_t* data, uint32_t data_len);

/**
 * Receive a message from the bus.
 * @param timeout_ms  Timeout in ms (0 = no timeout / blocking).
 * @param out_message On success, receives a message handle. Must be freed with ipmb_message_free.
 * @return 0 on success, -1 on error/timeout.
 */
int32_t ipmb_recv(ipmb_Receiver* receiver, ipmb_Message** out_message, uint32_t timeout_ms);

/** Get the format field from a received message. */
uint16_t ipmb_message_format(const ipmb_Message* msg);

/**
 * Get a pointer to the message data and its length.
 * The pointer is valid until ipmb_message_free is called.
 */
const uint8_t* ipmb_message_data(const ipmb_Message* msg, uint32_t* out_len);

/** Free a message returned by ipmb_recv. */
void ipmb_message_free(ipmb_Message* msg);

/** Free a sender returned by ipmb_join. */
void ipmb_sender_free(ipmb_Sender* sender);

/** Free a receiver returned by ipmb_join. */
void ipmb_receiver_free(ipmb_Receiver* receiver);

#ifdef __cplusplus
}
#endif

#endif /* IPMB_H */
