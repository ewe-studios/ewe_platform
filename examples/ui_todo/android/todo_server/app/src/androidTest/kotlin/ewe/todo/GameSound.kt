package ewe.todo

import android.graphics.SurfaceTexture
import android.view.Surface
import ewe.ui.android.attach
import android.media.AudioFormat
import android.media.AudioTrack
import io.github.ewe_studios.ewe.core.NativePlatform
import io.github.ewe_studios.ewe.todo.MathalleyGame
import io.github.ewe_studios.ewe.todo.createLocalTodoCore
import kotlinx.coroutines.delay
import kotlinx.coroutines.withTimeout
import kotlin.math.abs

internal suspend fun verifyGameSound() {
    val rate = 48000
    val frames = 4096u
    createLocalTodoCore(NativePlatform.ANDROID).use { core ->
        MathalleyGame(core, 13uL, rate.toUInt()).use { game ->
            MathalleyGame(core, 13uL, rate.toUInt()).use { other ->
                val music = game.samples(0uL, frames)
                check(music.any { abs(it) > 0.001f })
                check(other.samples(0uL, frames) == music)
                val texture = SurfaceTexture(false)
                texture.setDefaultBufferSize(64, 64)
                val nativeSurface = Surface(texture)
                try {
                    core.graphics().use { gpu ->
                        gpu.attach("sound-check", nativeSurface, 64u, 64u).use { surface ->
                            try {
                                game.tap(195f, 422f, 390f, 844f)
                                game.draw(surface, 0uL)
                            } finally { surface.remove() }
                        }
                    }
                } finally {
                    nativeSurface.release()
                    texture.release()
                }
                check(game.samples(4096uL, frames) != other.samples(4096uL, frames))
                game.tap(292f, 798f, 390f, 844f)
                game.active(false)
                check(game.samples(4096uL, frames).all { it == 0f })
                game.active(true)
                check(game.samples(8192uL, frames).all { it == 0f })
                game.stopSound()
                check(runCatching { game.samples(12288uL, frames) }.isFailure)
                val samples = other.samples(12288uL, frames).toFloatArray()
                check(samples.any { abs(it) > 0.001f })
                val format = AudioFormat.Builder().setSampleRate(rate).setChannelMask(AudioFormat.CHANNEL_OUT_MONO)
                    .setEncoding(AudioFormat.ENCODING_PCM_FLOAT).build()
                val track = AudioTrack.Builder().setAudioFormat(format).setTransferMode(AudioTrack.MODE_STATIC)
                    .setBufferSizeInBytes(samples.size * Float.SIZE_BYTES).build()
                try {
                    check(track.write(samples, 0, samples.size, AudioTrack.WRITE_BLOCKING) == samples.size)
                    track.play()
                    withTimeout(5000) { while (track.playbackHeadPosition == 0) delay(20) }
                } finally { track.release() }
                other.stopSound()
            }
        }
    }
}
