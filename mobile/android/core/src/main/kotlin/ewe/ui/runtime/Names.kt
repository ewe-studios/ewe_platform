package ewe.ui.runtime

import ewe.ui.*

/** Names used in reports, event dispatch and reference snapshots. */
internal object Names {
    val nodeKind =
        mapOf(
            NodeKind.UNKNOWN to "UNKNOWN",
            NodeKind.STACK to "STACK",
            NodeKind.TEXT to "TEXT",
            NodeKind.IMAGE to "IMAGE",
            NodeKind.BUTTON to "BUTTON",
            NodeKind.TEXT_INPUT to "TEXT_INPUT",
            NodeKind.TOGGLE to "TOGGLE",
            NodeKind.LIST to "LIST",
            NodeKind.SCROLL to "SCROLL",
            NodeKind.SPACER to "SPACER",
            NodeKind.OVERLAY to "OVERLAY",
            NodeKind.MODAL to "MODAL",
            NodeKind.BOTTOM_SHEET to "BOTTOM_SHEET",
            NodeKind.NAVIGATION_TITLE to "NAVIGATION_TITLE",
            NodeKind.TOOLBAR_BUTTON to "TOOLBAR_BUTTON",
            NodeKind.SEGMENTED_CONTROL to "SEGMENTED_CONTROL",
            NodeKind.INSET_LIST to "INSET_LIST",
            NodeKind.CHECKLIST_ROW to "CHECKLIST_ROW",
            NodeKind.CHECKMARK_BUTTON to "CHECKMARK_BUTTON",
            NodeKind.DESTRUCTIVE_BUTTON to "DESTRUCTIVE_BUTTON",
            NodeKind.LIST_FOOTER to "LIST_FOOTER",
            NodeKind.CONFIRMATION_DIALOG to "CONFIRMATION_DIALOG",
            NodeKind.SHEET_TITLE to "SHEET_TITLE",
            NodeKind.SUBMIT_BUTTON to "SUBMIT_BUTTON",
            NodeKind.CANCEL_BUTTON to "CANCEL_BUTTON",
        )
    val propId =
        mapOf(
            PropId.UNKNOWN to "UNKNOWN",
            PropId.PADDING to "PADDING",
            PropId.SPACING to "SPACING",
            PropId.ALIGN to "ALIGN",
            PropId.COLOR to "COLOR",
            PropId.FONT to "FONT",
            PropId.ENABLED to "ENABLED",
            PropId.A11Y_LABEL to "A11Y_LABEL",
            PropId.AXIS to "AXIS",
            PropId.VALUE to "VALUE",
            PropId.CHECKED to "CHECKED",
            PropId.SOURCE to "SOURCE",
            PropId.PLACEHOLDER to "PLACEHOLDER",
            PropId.BACKGROUND to "BACKGROUND",
            PropId.WIDTH to "WIDTH",
            PropId.HEIGHT to "HEIGHT",
            PropId.HIDDEN to "HIDDEN",
            PropId.UTILITY_CLASSES to "UTILITY_CLASSES",
        )
    val eventId =
        mapOf(
            EventId.UNKNOWN to "UNKNOWN",
            EventId.TAP to "TAP",
            EventId.CHANGE to "CHANGE",
            EventId.SUBMIT to "SUBMIT",
            EventId.FOCUS to "FOCUS",
            EventId.BLUR to "BLUR",
        )
    val behaviourId =
        mapOf(
            BehaviourId.UNKNOWN to "UNKNOWN",
            BehaviourId.DIALOG to "DIALOG",
            BehaviourId.DISMISS to "DISMISS",
            BehaviourId.SCROLL_LOCK to "SCROLL_LOCK",
            BehaviourId.TRANSITION to "TRANSITION",
        )
    val nativeCommandId =
        mapOf(
            NativeCommandId.UNKNOWN to "UNKNOWN",
            NativeCommandId.HAPTIC to "HAPTIC",
            NativeCommandId.OPEN_URL to "OPEN_URL",
            NativeCommandId.SHARE to "SHARE",
        )
    val liveKind =
        mapOf(
            LiveKind.Stream to "Stream",
            LiveKind.Data to "Data",
        )
    val liveTransport =
        mapOf(
            LiveTransport.Local to "Local",
            LiveTransport.Sse to "Sse",
            LiveTransport.WebSocket to "WebSocket",
            LiveTransport.Http to "Http",
        )
    val alignment =
        mapOf(
            Alignment.Start to "Start",
            Alignment.Center to "Center",
            Alignment.End to "End",
            Alignment.Stretch to "Stretch",
        )
    val axis =
        mapOf(
            Axis.Vertical to "Vertical",
            Axis.Horizontal to "Horizontal",
        )
    val capsStatus =
        mapOf(
            CapsStatus.Ok to "Ok",
            CapsStatus.UpdateRequired to "UpdateRequired",
        )
    val errorCode =
        mapOf(
            ErrorCode.Unknown to "Unknown",
            ErrorCode.BadRequest to "BadRequest",
            ErrorCode.UnknownAction to "UnknownAction",
            ErrorCode.UnavailableOrigin to "UnavailableOrigin",
            ErrorCode.UnsupportedAction to "UnsupportedAction",
            ErrorCode.StateRejected to "StateRejected",
            ErrorCode.StateExpired to "StateExpired",
            ErrorCode.RateLimited to "RateLimited",
            ErrorCode.ActionFailed to "ActionFailed",
            ErrorCode.UpdateRequired to "UpdateRequired",
            ErrorCode.UnsupportedPlatformOp to "UnsupportedPlatformOp",
        )
}
