package ewe.ui.android

import android.content.res.AssetManager
import io.github.ewe_studios.ewe.core.CoreException
import io.github.ewe_studios.ewe.core.HostResourceFile
import io.github.ewe_studios.ewe.core.HostResourceSource
import java.io.FileNotFoundException
import java.io.IOException
import java.io.InputStream

/** Opens package files with the application's asset manager. */
class AndroidResourceSource(private val assets: AssetManager) : HostResourceSource {
    override fun open(name: String): HostResourceFile? = try {
        AssetResourceFile(assets.open("EweResources/$name", AssetManager.ACCESS_STREAMING))
    } catch (_: FileNotFoundException) {
        null
    } catch (error: IOException) {
        throw CoreException.Transport(error.toString())
    }
}

private class AssetResourceFile(private val input: InputStream) : HostResourceFile {
    override fun read(capacity: UInt): ByteArray = try {
        val bytes = ByteArray(capacity.coerceAtMost(Int.MAX_VALUE.toUInt()).toInt())
        val used = input.read(bytes)
        when {
            used < 0 -> byteArrayOf()
            used == bytes.size -> bytes
            else -> bytes.copyOf(used)
        }
    } catch (error: IOException) {
        throw CoreException.Transport(error.toString())
    }

    override fun close() {
        try { input.close() }
        catch (error: IOException) { throw CoreException.Transport(error.toString()) }
    }
}
