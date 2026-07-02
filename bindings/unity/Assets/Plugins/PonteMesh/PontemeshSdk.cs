using System;
using System.Runtime.InteropServices;

namespace PonteMesh
{
    public sealed class PontemeshClient : IDisposable
    {
        private IntPtr client;

        public PontemeshClient(string originUrl, string applicationToken)
        {
            var status = pontemesh_client_create(originUrl, applicationToken, out client);
            if (status != PontemeshStatus.PONTEMESH_OK)
            {
                throw new InvalidOperationException("Could not create Ponte Mesh client: " + status);
            }
        }

        public void SyncObject(string bucket, string key, string destination)
        {
            var status = pontemesh_client_sync_object(client, bucket, key, destination);
            if (status != PontemeshStatus.PONTEMESH_OK)
            {
                throw new InvalidOperationException("Could not sync Ponte Mesh object: " + status);
            }
        }

        public void Dispose()
        {
            if (client != IntPtr.Zero)
            {
                pontemesh_client_free(client);
                client = IntPtr.Zero;
            }
        }

        [DllImport("pontemesh_sdk", CallingConvention = CallingConvention.Cdecl)]
        private static extern PontemeshStatus pontemesh_client_create(string originUrl, string applicationToken, out IntPtr client);

        [DllImport("pontemesh_sdk", CallingConvention = CallingConvention.Cdecl)]
        private static extern PontemeshStatus pontemesh_client_sync_object(IntPtr client, string bucket, string key, string destination);

        [DllImport("pontemesh_sdk", CallingConvention = CallingConvention.Cdecl)]
        private static extern void pontemesh_client_free(IntPtr client);
    }

    internal enum PontemeshStatus
    {
        PONTEMESH_OK = 0,
        PONTEMESH_INVALID_ARGUMENT = 1,
        PONTEMESH_ORIGIN_REQUEST_FAILED = 2,
        PONTEMESH_ACCESS_DENIED = 3,
        PONTEMESH_HASH_MISMATCH = 4,
        PONTEMESH_NO_SOURCE_AVAILABLE = 5,
        PONTEMESH_IO_ERROR = 6,
        PONTEMESH_CANCELLED = 7,
        PONTEMESH_INTERNAL_ERROR = 255
    }
}

