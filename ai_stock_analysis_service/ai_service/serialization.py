"""
Serialization utilities for Rust <-> Python communication.
Content-Type header determines format automatically.
- application/json    -> JSON  (candle_count <= 1000)
- application/msgpack -> MsgPack (candle_count > 1000)
"""
import json
import msgpack
from ai_service.constants import MSGPACK_CANDLE_THRESHOLD


def decode_request(content_type: str, body: bytes) -> dict:
    """
    Decode incoming request body based on Content-Type header.
    Raises ValueError for unsupported content types.
    """
    if content_type == "application/msgpack":
        return msgpack.unpackb(body, raw=False)
    return json.loads(body)


def encode_response(data: dict, candle_count: int) -> tuple[bytes, str]:
    """
    Encode response body. Choose format based on candle count to mirror Rust logic.
    Returns (encoded_bytes, content_type).
    """
    if candle_count > MSGPACK_CANDLE_THRESHOLD:
        return msgpack.packb(data, use_bin_type=True), "application/msgpack"
    return json.dumps(data).encode(), "application/json"