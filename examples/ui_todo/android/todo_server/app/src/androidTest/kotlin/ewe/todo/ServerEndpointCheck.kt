package ewe.todo

import android.content.Context

internal fun verifyServerEndpoint(context: Context) {
    val file = "todo-server-endpoint-check"
    context.deleteSharedPreferences(file)
    try {
        val settings = context.getSharedPreferences(file, Context.MODE_PRIVATE)
        val configured = "http://127.0.0.1:6780/"
        val first = "http://127.0.0.1:6781/"
        val second = "https://example.test/todo/"
        check(resolveTodoServerEndpoint(settings, null, configured).toString() == configured)
        check(resolveTodoServerEndpoint(settings, "invalid", configured).toString() == configured)
        check(resolveTodoServerEndpoint(settings, first.removeSuffix("/"), configured).toString() == first)
        val reopened = context.getSharedPreferences(file, Context.MODE_PRIVATE)
        check(resolveTodoServerEndpoint(reopened, null, configured).toString() == first)
        check(resolveTodoServerEndpoint(reopened, "ftp://example.test", configured).toString() == first)
        check(resolveTodoServerEndpoint(reopened, second, configured).toString() == second)
        check(resolveTodoServerEndpoint(reopened, null, configured).toString() == second)
    } finally {
        context.deleteSharedPreferences(file)
    }
}
