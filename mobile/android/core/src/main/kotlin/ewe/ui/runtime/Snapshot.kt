package ewe.ui.runtime

import kotlinx.serialization.json.*

fun NodeStore.snapshotJson(): String = buildJsonObject {
    putJsonArray("nodes") {
        nodes
            .sortedBy { it.id }
            .forEach { n ->
                addJsonObject {
                    put("id", n.id.toLong())
                    put("kind", Names.nodeKind[n.kind] ?: "UNKNOWN")
                    put("text", n.text)
                    putJsonObject("props") {
                        n.props.forEach { (k, v) -> put(Names.propId.getValue(k), v.label) }
                    }
                    putJsonArray("children") { n.children.forEach { add(it.toLong()) } }
                    putJsonObject("actions") {
                        n.actions.forEach { (k, v) -> Names.eventId[k]?.let { put(it, v) } }
                    }
                    put("state_len", n.state.size)
                    put(
                        "live",
                        n.live?.let { v ->
                            buildJsonObject {
                                put("kind", Names.liveKind[v.kind])
                                put("channel_id", JsonPrimitive(v.stream))
                                put("transport", Names.liveTransport[v.transport])
                                put("endpoint", v.endpoint)
                                put("resume_from", JsonPrimitive(v.resume))
                            }
                        } ?: JsonNull,
                    )
                    put(
                        "web_view",
                        n.webView?.let { v ->
                            buildJsonObject {
                                put("parent_id", v.parent.toLong())
                                put("url", v.url)
                            }
                        } ?: JsonNull,
                    )
                    putJsonArray("behaviours") { n.behaviours.sorted().forEach { add(it) } }
                }
            }
    }
    put("reports", JsonArray(reports))
    put("commands", JsonArray(sentCommands))
    put("errors", JsonArray(errors))
    put("data", JsonArray(data))
    if (caps.isNotEmpty()) put("caps", JsonArray(caps))
}
    .toString()
