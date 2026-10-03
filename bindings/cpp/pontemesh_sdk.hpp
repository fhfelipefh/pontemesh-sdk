#pragma once

#include <optional>
#include <stdexcept>
#include <string>
#include <vector>

#include "../c/include/pontemesh_sdk.h"

namespace pontemesh {

struct SoftwareUpdate {
    std::string bucket;
    std::string software_id;
    std::string versioning_scheme;
    std::string current_version;
    std::string latest_version;
    bool has_update = false;
    std::string target_object_key;
    uint64_t size_bytes = 0;
    std::string manifest_id;
    bool mandatory = false;
};

class Client {
public:
    Client(const char* origin_url, const char* application_token) {
        PontemeshStatus status = pontemesh_client_create(origin_url, application_token, &client_);
        if (status != PONTEMESH_OK) {
            throw std::runtime_error("pontemesh_client_create failed");
        }
    }

    ~Client() {
        pontemesh_client_free(client_);
    }

    Client(const Client&) = delete;
    Client& operator=(const Client&) = delete;

    void sync_object(const char* bucket, const char* key, const char* destination) {
        PontemeshStatus status = pontemesh_client_sync_object(client_, bucket, key, destination);
        if (status != PONTEMESH_OK) {
            throw std::runtime_error(last_error("pontemesh_client_sync_object failed"));
        }
    }

    PontemeshTransferSummary sync_object_with_summary(
        const char* bucket,
        const char* key,
        const char* destination
    ) {
        PontemeshTransferSummary summary{};
        PontemeshStatus status = pontemesh_client_sync_object_with_summary(
            client_,
            bucket,
            key,
            destination,
            &summary
        );
        if (status != PONTEMESH_OK) {
            throw std::runtime_error(last_error("pontemesh_client_sync_object_with_summary failed"));
        }
        return summary;
    }

    void enable_p2p(const char* listen_addr = nullptr) {
        PontemeshStatus status = pontemesh_client_enable_p2p(client_, listen_addr);
        if (status != PONTEMESH_OK) {
            throw std::runtime_error(last_error("pontemesh_client_enable_p2p failed"));
        }
    }

    std::optional<SoftwareUpdate> check_software_update(
        const char* bucket,
        const char* software_id,
        const char* current_version = nullptr,
        const char* channel = nullptr
    ) {
        PontemeshSoftwareUpdate* raw_update = nullptr;
        PontemeshStatus status = pontemesh_client_check_software_update(
            client_,
            bucket,
            software_id,
            current_version,
            channel,
            &raw_update
        );
        if (status != PONTEMESH_OK) {
            throw std::runtime_error(last_error("pontemesh_client_check_software_update failed"));
        }
        if (!raw_update) {
            return std::nullopt;
        }

        SoftwareUpdate update;
        if (raw_update->bucket) update.bucket = raw_update->bucket;
        if (raw_update->software_id) update.software_id = raw_update->software_id;
        if (raw_update->versioning_scheme) update.versioning_scheme = raw_update->versioning_scheme;
        if (raw_update->current_version) update.current_version = raw_update->current_version;
        if (raw_update->latest_version) update.latest_version = raw_update->latest_version;
        update.has_update = raw_update->has_update != 0;
        if (raw_update->target_object_key) update.target_object_key = raw_update->target_object_key;
        update.size_bytes = raw_update->size_bytes;
        if (raw_update->manifest_id) update.manifest_id = raw_update->manifest_id;
        update.mandatory = raw_update->mandatory != 0;

        pontemesh_software_update_free(raw_update);
        return update;
    }

private:
    std::string last_error(const char* fallback) const {
        std::vector<char> buffer(1024);
        PontemeshStatus status = pontemesh_client_get_last_error(
            client_,
            buffer.data(),
            buffer.size()
        );
        if (status != PONTEMESH_OK || buffer[0] == '\0') {
            return fallback;
        }
        return std::string(buffer.data());
    }

    PontemeshClient* client_ = nullptr;
};

} // namespace pontemesh
