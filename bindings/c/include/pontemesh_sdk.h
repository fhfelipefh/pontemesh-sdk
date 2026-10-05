#ifndef PONTEMESH_SDK_H
#define PONTEMESH_SDK_H

#include <stddef.h>
#include <stdint.h>

#ifdef _WIN32
#define PONTEMESH_SDK_EXPORT __declspec(dllexport)
#else
#define PONTEMESH_SDK_EXPORT
#endif

#ifdef __cplusplus
extern "C" {
#endif

typedef struct PontemeshClient PontemeshClient;

typedef enum PontemeshStatus {
    PONTEMESH_OK = 0,
    PONTEMESH_INVALID_ARGUMENT = 1,
    PONTEMESH_ORIGIN_REQUEST_FAILED = 2,
    PONTEMESH_ACCESS_DENIED = 3,
    PONTEMESH_HASH_MISMATCH = 4,
    PONTEMESH_NO_SOURCE_AVAILABLE = 5,
    PONTEMESH_IO_ERROR = 6,
    PONTEMESH_CANCELLED = 7,
    PONTEMESH_SUSPENDED = 8,
    PONTEMESH_PATH_NOT_ALLOWED = 9,
    PONTEMESH_INTERNAL_ERROR = 255
} PontemeshStatus;

typedef void (*PontemeshProgressCallback)(
    uint32_t fragment_index,
    uint64_t bytes_downloaded,
    uint64_t total_bytes,
    const char* source_type,
    void* user_data
);

typedef struct PontemeshTransferSummary {
    uint64_t bytes_from_peer;
    uint64_t bytes_from_replica;
    uint64_t bytes_from_origin;
    uint64_t fragments_from_peer;
    uint64_t fragments_from_replica;
    uint64_t fragments_from_origin;
    uint64_t peer_failures;
    uint64_t peer_hash_failures;
    uint64_t peer_rejected_fragments;
    uint64_t fallback_activations;
} PontemeshTransferSummary;

PONTEMESH_SDK_EXPORT PontemeshStatus pontemesh_client_create(
    const char* origin_url,
    const char* application_token,
    PontemeshClient** out_client
);

PONTEMESH_SDK_EXPORT PontemeshStatus pontemesh_client_sync_object(
    PontemeshClient* client,
    const char* bucket,
    const char* key,
    const char* destination
);

PONTEMESH_SDK_EXPORT PontemeshStatus pontemesh_client_enable_p2p(
    PontemeshClient* client,
    const char* listen_addr
);

PONTEMESH_SDK_EXPORT PontemeshStatus pontemesh_client_sync_object_with_progress(
    PontemeshClient* client,
    const char* bucket,
    const char* key,
    const char* destination,
    PontemeshProgressCallback callback,
    void* user_data
);

PONTEMESH_SDK_EXPORT PontemeshStatus pontemesh_client_sync_object_with_summary(
    PontemeshClient* client,
    const char* bucket,
    const char* key,
    const char* destination,
    PontemeshTransferSummary* out_summary
);

PONTEMESH_SDK_EXPORT PontemeshStatus pontemesh_client_sync_object_with_summary_and_progress(
    PontemeshClient* client,
    const char* bucket,
    const char* key,
    const char* destination,
    PontemeshTransferSummary* out_summary,
    PontemeshProgressCallback callback,
    void* user_data
);

PONTEMESH_SDK_EXPORT PontemeshStatus pontemesh_client_set_active(
    PontemeshClient* client,
    int32_t active
);

PONTEMESH_SDK_EXPORT PontemeshStatus pontemesh_client_is_active(
    PontemeshClient* client,
    int32_t* out_active
);

PONTEMESH_SDK_EXPORT PontemeshStatus pontemesh_client_suspend(
    PontemeshClient* client
);

PONTEMESH_SDK_EXPORT PontemeshStatus pontemesh_client_resume(
    PontemeshClient* client
);

PONTEMESH_SDK_EXPORT PontemeshStatus pontemesh_client_add_allowed_directory(
    PontemeshClient* client,
    const char* path
);

PONTEMESH_SDK_EXPORT PontemeshStatus pontemesh_client_clear_allowed_directories(
    PontemeshClient* client
);

PONTEMESH_SDK_EXPORT PontemeshStatus pontemesh_client_get_last_error(
    PontemeshClient* client,
    char* buffer,
    size_t buffer_len
);

typedef struct PontemeshSoftwareUpdate {
    char* bucket;
    char* software_id;
    char* versioning_scheme;
    char* current_version;
    char* latest_version;
    int32_t has_update;
    char* target_object_key;
    uint64_t size_bytes;
    char* manifest_id;
    int32_t mandatory;
} PontemeshSoftwareUpdate;

PONTEMESH_SDK_EXPORT PontemeshStatus pontemesh_client_check_software_update(
    PontemeshClient* client,
    const char* bucket,
    const char* software_id,
    const char* current_version,
    const char* channel,
    PontemeshSoftwareUpdate** out_update
);

PONTEMESH_SDK_EXPORT void pontemesh_software_update_free(PontemeshSoftwareUpdate* update);

PONTEMESH_SDK_EXPORT void pontemesh_client_free(PontemeshClient* client);

#ifdef __cplusplus
}
#endif

#endif
