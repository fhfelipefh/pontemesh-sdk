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

    [StructLayout(LayoutKind.Sequential)]
    internal struct NativeSoftwareUpdate
    {
        public IntPtr Bucket;
        public IntPtr SoftwareId;
        public IntPtr VersioningScheme;
        public IntPtr CurrentVersion;
        public IntPtr LatestVersion;
        public int HasUpdate;
        public IntPtr TargetObjectKey;
        public ulong SizeBytes;
        public IntPtr ManifestId;
        public int Mandatory;
    }

    public sealed class PontemeshSoftwareUpdate
    {
        public string Bucket { get; set; } = string.Empty;
        public string SoftwareId { get; set; } = string.Empty;
        public string VersioningScheme { get; set; } = string.Empty;
        public string CurrentVersion { get; set; }
        public string LatestVersion { get; set; } = string.Empty;
        public bool HasUpdate { get; set; }
        public string TargetObjectKey { get; set; } = string.Empty;
        public ulong SizeBytes { get; set; }
        public string ManifestId { get; set; }
        public bool Mandatory { get; set; }
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

        public void SetActive(bool active)
        {
            var status = pontemesh_client_set_active(client, active ? 1 : 0);
            ThrowIfError(status);
        }

        public bool IsActive()
        {
            return pontemesh_client_is_active(client) != 0;
        }

        public void Suspend()
        {
            var status = pontemesh_client_suspend(client);
            ThrowIfError(status);
        }

        public void Resume()
        {
            var status = pontemesh_client_resume(client);
            ThrowIfError(status);
        }

        public void AddAllowedDirectory(string directoryPath)
        {
            var status = pontemesh_client_add_allowed_directory(client, directoryPath);
            ThrowIfError(status);
        }

        public void ClearAllowedDirectories()
        {
            var status = pontemesh_client_clear_allowed_directories(client);
            ThrowIfError(status);
        }

        public PontemeshSoftwareUpdate CheckSoftwareUpdate(
            string bucket,
            string softwareId,
            string currentVersion = null,
            string channel = null
        )
        {
            var status = pontemesh_client_check_software_update(
                client,
                bucket,
                softwareId,
                currentVersion,
                channel,
                out var updatePtr
            );
            ThrowIfError(status);
            if (updatePtr == IntPtr.Zero)
            {
                return null;
            }

            try
            {
                var native = (NativeSoftwareUpdate)Marshal.PtrToStructure(updatePtr, typeof(NativeSoftwareUpdate));
                return new PontemeshSoftwareUpdate
                {
                    Bucket = Marshal.PtrToStringUTF8(native.Bucket) ?? string.Empty,
                    SoftwareId = Marshal.PtrToStringUTF8(native.SoftwareId) ?? string.Empty,
                    VersioningScheme = Marshal.PtrToStringUTF8(native.VersioningScheme) ?? string.Empty,
                    CurrentVersion = native.CurrentVersion != IntPtr.Zero ? Marshal.PtrToStringUTF8(native.CurrentVersion) : null,
                    LatestVersion = Marshal.PtrToStringUTF8(native.LatestVersion) ?? string.Empty,
                    HasUpdate = native.HasUpdate != 0,
                    TargetObjectKey = Marshal.PtrToStringUTF8(native.TargetObjectKey) ?? string.Empty,
                    SizeBytes = native.SizeBytes,
                    ManifestId = native.ManifestId != IntPtr.Zero ? Marshal.PtrToStringUTF8(native.ManifestId) : null,
                    Mandatory = native.Mandatory != 0,
                };
            }
            finally
            {
                pontemesh_software_update_free(updatePtr);
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
        private static extern PontemeshStatus pontemesh_client_set_active(IntPtr client, int active);

        [DllImport("pontemesh_sdk", CallingConvention = CallingConvention.Cdecl)]
        private static extern int pontemesh_client_is_active(IntPtr client);

        [DllImport("pontemesh_sdk", CallingConvention = CallingConvention.Cdecl)]
        private static extern PontemeshStatus pontemesh_client_suspend(IntPtr client);

        [DllImport("pontemesh_sdk", CallingConvention = CallingConvention.Cdecl)]
        private static extern PontemeshStatus pontemesh_client_resume(IntPtr client);

        [DllImport("pontemesh_sdk", CallingConvention = CallingConvention.Cdecl)]
        private static extern PontemeshStatus pontemesh_client_add_allowed_directory(IntPtr client, string directoryPath);

        [DllImport("pontemesh_sdk", CallingConvention = CallingConvention.Cdecl)]
        private static extern PontemeshStatus pontemesh_client_clear_allowed_directories(IntPtr client);

        [DllImport("pontemesh_sdk", CallingConvention = CallingConvention.Cdecl)]
        private static extern PontemeshStatus pontemesh_client_get_last_error(IntPtr client, byte[] buffer, UIntPtr bufferLen);

        [DllImport("pontemesh_sdk", CallingConvention = CallingConvention.Cdecl)]
        private static extern void pontemesh_client_free(IntPtr client);

        [DllImport("pontemesh_sdk", CallingConvention = CallingConvention.Cdecl)]
        private static extern PontemeshStatus pontemesh_client_check_software_update(
            IntPtr client,
            string bucket,
            string softwareId,
            string currentVersion,
            string channel,
            out IntPtr update
        );

        [DllImport("pontemesh_sdk", CallingConvention = CallingConvention.Cdecl)]
        private static extern void pontemesh_software_update_free(IntPtr update);

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
        PONTEMESH_SUSPENDED = 8,
        PONTEMESH_PATH_NOT_ALLOWED = 9,
        PONTEMESH_INTERNAL_ERROR = 255
    }
}
