#pragma once

#include <stdexcept>
#include <string>
#include <vector>

#include "../c/include/pontemesh_sdk.h"

namespace pontemesh {

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
