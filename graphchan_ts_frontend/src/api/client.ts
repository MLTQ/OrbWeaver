import axios from 'axios';
import {
    type CreatePostInput,
    type CreateThreadInput,
    type FileView,
    type HealthResponse,
    type ImportResponse,
    type PeerView,
    type PostView,
    type ThreadDetails,
    type ThreadSummary,
} from './types';

// Backend is running on localhost:8080 by default
const API_BASE_URL = 'http://localhost:8080';

const client = axios.create({
    baseURL: API_BASE_URL,
    timeout: 30000,
});

export const api = {
    // Health & System
    getHealth: async () => {
        const { data } = await client.get<HealthResponse>('/health');
        return data;
    },

    // Threads
    listThreads: async (limit = 50) => {
        const { data } = await client.get<ThreadSummary[]>('/threads', {
            params: { limit },
        });
        return data;
    },

    createThread: async (input: CreateThreadInput) => {
        const { data } = await client.post<ThreadDetails>('/threads', input);
        return data;
    },

    getThread: async (id: string) => {
        const { data } = await client.get<ThreadDetails>(`/threads/${id}`);
        return data;
    },

    // Posts
    createPost: async (threadId: string, input: CreatePostInput) => {
        const { data } = await client.post<{ post: PostView }>(
            `/threads/${threadId}/posts`,
            input
        );
        return data.post;
    },

    // Files
    uploadFile: async (postId: string, file: File) => {
        const formData = new FormData();
        formData.append('file', file);
        const { data } = await client.post<FileView>(
            `/posts/${postId}/files`,
            formData,
            {
                headers: {
                    'Content-Type': 'multipart/form-data',
                },
            }
        );
        return data;
    },

    getFileUrl: (fileId: string) => {
        return `${API_BASE_URL}/files/${fileId}`;
    },

    // Peers
    listPeers: async () => {
        const { data } = await client.get<PeerView[]>('/peers');
        return data;
    },

    addPeer: async (friendcode: string) => {
        const { data } = await client.post<PeerView>('/peers', { friendcode });
        return data;
    },

    importThread: async (url: string) => {
        const { data } = await client.post<ImportResponse>('/import', { url });
        return data;
    },

    getSelfPeer: async () => {
        const { data } = await client.get<PeerView | null>('/peers/self');
        return data;
    },
};
