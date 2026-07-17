using System;
using System.Runtime.InteropServices;
using System.Text;

namespace PonteMesh
{
    [StructLayout(LayoutKind.Sequential)]
    public struct PontemeshTransferSummary
    {
        public ulong BytesFromPeer;
        public ulong BytesFromReplica;
        public ulong BytesFromOrigin;
        public ulong FragmentsFromPeer;
        public ulong FragmentsFromReplica;
        public ulong FragmentsFromOrigin;
        public ulong PeerFailures;
        public ulong PeerHashFailures;
        public ulong PeerRejectedFragments;
        public ulong FallbackActivations;
    }

    public delegate void PontemeshProgressCallback(
        uint fragmentIndex,
        ulong bytesDownloaded,
        ulong totalBytes,
        string sourceType
    );

    public sealed class PontemeshClient : IDisposable
    {
        private IntPtr client;
        private NativeProgressCallback nativeProgressCallback = NoopNativeProgress;
        private PontemeshProgressCallback managedProgressCallback = NoopProgress;

        public PontemeshClient(string originUrl, string applicationToken)
        {
            var status = pontemesh_client_create(originUrl, applicationToken, out client);
            ThrowIfError(status);
        }

        public void SyncObject(string bucket, string key, string destination)
        {
            var status = pontemesh_client_sync_object(client, bucket, key, destination);
            ThrowIfError(status);
        }

        public PontemeshTransferSummary SyncObjectWithSummary(string bucket, string key, string destination)
        {
            var status = pontemesh_client_sync_object_with_summary(
                client,
                bucket,
                key,
                destination,
                out var summary
            );
            ThrowIfError(status);
            return summary;
        }

        public PontemeshTransferSummary SyncObjectWithSummary(
            string bucket,
            string key,
            string destination,
            PontemeshProgressCallback progress
        )
        {
            if (progress == null)
            {
                throw new ArgumentNullException(nameof(progress));
            }
            managedProgressCallback = progress;
            nativeProgressCallback = OnProgress;
            var status = pontemesh_client_sync_object_with_summary_and_progress(
                client,
                bucket,
                key,
                destination,
                out var summary,
                nativeProgressCallback,
                IntPtr.Zero
            );
            nativeProgressCallback = NoopNativeProgress;
            managedProgressCallback = NoopProgress;
            ThrowIfError(status);
            return summary;
        }

        public void EnableP2p(string listenAddr)
        {
            var status = pontemesh_client_enable_p2p(client, listenAddr);
            ThrowIfError(status);
        }

        public void Dispose()
        {
            if (client != IntPtr.Zero)
            {
                pontemesh_client_free(client);
                client = IntPtr.Zero;
            }
        }

        private void ThrowIfError(PontemeshStatus status)
        {
            if (status == PontemeshStatus.PONTEMESH_OK)
            {
                return;
            }
            throw new InvalidOperationException(ReadLastError(status));
        }

        private string ReadLastError(PontemeshStatus status)
        {
            var buffer = new byte[2048];
            var readStatus = pontemesh_client_get_last_error(client, buffer, (UIntPtr)buffer.Length);
            if (readStatus != PontemeshStatus.PONTEMESH_OK || buffer[0] == 0)
            {
                return "Ponte Mesh SDK failed with status " + status;
            }
            var length = Array.IndexOf<byte>(buffer, 0);
            if (length < 0)
            {
                length = buffer.Length;
            }
            return Encoding.UTF8.GetString(buffer, 0, length);
        }

        private void OnProgress(
            uint fragmentIndex,
            ulong bytesDownloaded,
            ulong totalBytes,
            IntPtr sourceType,
            IntPtr userData
        )
        {
            var source = Marshal.PtrToStringAnsi(sourceType) ?? string.Empty;
            managedProgressCallback?.Invoke(fragmentIndex, bytesDownloaded, totalBytes, source);
        }

        private static void NoopProgress(
            uint fragmentIndex,
            ulong bytesDownloaded,
            ulong totalBytes,
            string sourceType
        )
        {
        }

        private static void NoopNativeProgress(
            uint fragmentIndex,
            ulong bytesDownloaded,
            ulong totalBytes,
            IntPtr sourceType,
            IntPtr userData
        )
        {
        }

        [DllImport("pontemesh_sdk", CallingConvention = CallingConvention.Cdecl)]
        private static extern PontemeshStatus pontemesh_client_create(string originUrl, string applicationToken, out IntPtr client);

        [DllImport("pontemesh_sdk", CallingConvention = CallingConvention.Cdecl)]
        private static extern PontemeshStatus pontemesh_client_sync_object(IntPtr client, string bucket, string key, string destination);

        [DllImport("pontemesh_sdk", CallingConvention = CallingConvention.Cdecl)]
        private static extern PontemeshStatus pontemesh_client_sync_object_with_summary(
            IntPtr client,
            string bucket,
            string key,
            string destination,
            out PontemeshTransferSummary summary
        );

        [DllImport("pontemesh_sdk", CallingConvention = CallingConvention.Cdecl)]
        private static extern PontemeshStatus pontemesh_client_sync_object_with_summary_and_progress(
            IntPtr client,
            string bucket,
            string key,
            string destination,
            out PontemeshTransferSummary summary,
            NativeProgressCallback callback,
            IntPtr userData
        );

        [DllImport("pontemesh_sdk", CallingConvention = CallingConvention.Cdecl)]
        private static extern PontemeshStatus pontemesh_client_enable_p2p(IntPtr client, string listenAddr);

        [DllImport("pontemesh_sdk", CallingConvention = CallingConvention.Cdecl)]
        private static extern PontemeshStatus pontemesh_client_get_last_error(IntPtr client, byte[] buffer, UIntPtr bufferLen);

        [DllImport("pontemesh_sdk", CallingConvention = CallingConvention.Cdecl)]
        private static extern void pontemesh_client_free(IntPtr client);

        private delegate void NativeProgressCallback(
            uint fragmentIndex,
            ulong bytesDownloaded,
            ulong totalBytes,
            IntPtr sourceType,
            IntPtr userData
        );
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
