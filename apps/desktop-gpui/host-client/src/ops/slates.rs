// SPDX-License-Identifier: AGPL-3.0-only
use crate::protocol;
use crate::Done;

ops! {
    "slates.approve" => SlatesApproveRequest => Done,
    "slates.cancel" => SlatesCancelRequest => Done,
    "slates.event" => SlatesEventRequest => protocol::SlateEventAnswer,
    "slates.get" => SlatesGetRequest => protocol::SlatesGetAnswer,
    "slates.image" => SlatesImageRequest => protocol::SlatesImageAnswer,
    "slates.page" => SlatesPageRequest => protocol::SlatesPageAnswer,
    "slates.read" => SlatesReadRequest => protocol::SlateReadAnswer,
    "slates.resolve" => SlatesResolveRequest => protocol::SlatesResolveAnswer,
    "slates.revoke" => SlatesRevokeRequest => Done,
    "slates.shown" => SlatesShownRequest => Done,
    "slates.state" => SlatesStateRequest => protocol::SlateStateAnswer,
    "slates.subscribe" => SlatesSubscribeRequest => Done,
    "slates.unsubscribe" => SlatesUnsubscribeRequest => Done,
    "slates.write" => SlatesWriteRequest => protocol::SlateWriteAnswer,
}
