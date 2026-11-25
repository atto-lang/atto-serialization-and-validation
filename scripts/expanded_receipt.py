"""Structural binding for the approved base native receipt, not authentication."""
import hashlib
import json
from pathlib import Path
import re
import stat

MAX_RECEIPT = 65536
MAX_BASE_LOG = 16 * 1024 * 1024
FLAGS = (
    "actual_native_guest_executed", "registry_marker_observed",
    "full_suite_marker_observed", "public_registry_import_control_passed",
    "public_export_control_executed", "negative_private_entry_import_refused",
    "negative_private_storage_access_refused",
)
ARTIFACTS = {
    "native_log_sha256": "zorde-registry-native.log",
    "public_registry_import_log_sha256": "zorde-public-import-positive.log",
    "public_export_control_log_sha256": "zorde-public-export-control.log",
}
NEGATIVE = ("private-entry-import", "private-storage-access")


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate receipt key")
        result[key] = value
    return result


def read_bounded(path: Path, maximum: int) -> bytes:
    st = path.lstat()
    if not stat.S_ISREG(st.st_mode) or st.st_size > maximum:
        raise ValueError("unsafe or oversized receipt artifact")
    with path.open("rb") as stream:
        data = stream.read(maximum + 1)
    if len(data) > maximum:
        raise ValueError("oversized receipt artifact")
    return data


def verify_base(out: Path, head: str, tree: str, compiler: str) -> str:
    raw = read_bounded(out / "zorde-registry-native-receipt.json", MAX_RECEIPT)
    record = json.loads(raw, object_pairs_hook=unique_object)
    if not isinstance(record, dict) or record.get("schema") != "oreslang.zorde.source-local.native-execution/v1":
        raise ValueError("wrong base receipt schema")
    for key, want, width in (("source_head_sha", head, 40), ("source_root_tree_sha", tree, 40), ("compiler_executable_sha256", compiler, 64)):
        if not re.fullmatch(rf"[0-9a-f]{{{width}}}", want) or record.get(key) != want:
            raise ValueError("base receipt identity mismatch")
    if type(record.get("process_exit_code")) is not int or record["process_exit_code"] != 0:
        raise ValueError("base native exit status invalid")
    if any(record.get(flag) is not True for flag in FLAGS):
        raise ValueError("base native controls incomplete")
    if record.get("release_ready") is not False:
        raise ValueError("base receipt overclaims release readiness")
    neg = record.get("negative_diagnostic_sha256")
    if not isinstance(neg, dict) or set(neg) != set(NEGATIVE):
        raise ValueError("base negative controls incomplete")
    for key, name in ARTIFACTS.items():
        data = read_bounded(out / name, MAX_BASE_LOG)
        if hashlib.sha256(data).hexdigest() != record.get(key):
            raise ValueError("base log digest mismatch")
        if key == "native_log_sha256" and not all(marker in data.splitlines() for marker in (b"native Zorde compiled scalar registry tests passed", b"serialization/validation native tests passed")):
            raise ValueError("base native markers missing")
        if key == "public_export_control_log_sha256" and b"native Zorde public export control passed" not in data.splitlines():
            raise ValueError("base export marker missing")
    for fixture in NEGATIVE:
        data = read_bounded(out / ("zorde-" + fixture + "-denial.log"), MAX_RECEIPT)
        if not data or hashlib.sha256(data).hexdigest() != neg[fixture]:
            raise ValueError("base negative log mismatch")
    return hashlib.sha256(raw).hexdigest()
