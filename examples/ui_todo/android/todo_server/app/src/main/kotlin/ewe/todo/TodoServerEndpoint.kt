package ewe.todo

import android.content.SharedPreferences
import okhttp3.HttpUrl
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.HttpUrl.Companion.toHttpUrlOrNull

private const val ADDRESS_KEY = "endpoint"

internal fun resolveTodoServerEndpoint(
    settings: SharedPreferences,
    launchAddress: String?,
    configuredAddress: String,
): HttpUrl {
    launchAddress?.toHttpUrlOrNull()?.let { endpoint ->
        settings.edit().putString(ADDRESS_KEY, endpoint.toString()).apply()
        return endpoint
    }
    return settings.getString(ADDRESS_KEY, null)?.toHttpUrlOrNull() ?: configuredAddress.toHttpUrl()
}
