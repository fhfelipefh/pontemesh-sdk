#pragma once

#include <stdexcept>
#include <string>

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
            throw std::runtime_error("pontemesh_client_sync_object failed");
        }
    }

private:
    PontemeshClient* client_ = nullptr;
};

} // namespace pontemesh

