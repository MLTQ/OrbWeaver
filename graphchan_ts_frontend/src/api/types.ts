export interface ThreadSummary {
  id: string;
  title: string;
  creator_peer_id?: string;
  created_at: string;
  pinned: boolean;
  post_count: number;
  last_bump_at: string;
}

export interface ThreadDetails {
  thread: ThreadSummary;
  posts: PostView[];
}

export interface PostView {
  id: string;
  thread_id: string;
  author_peer_id?: string;
  body: string;
  created_at: string;
  updated_at?: string;
  files: FileView[];
  children: string[]; // IDs of child posts
  parents: string[]; // IDs of parent posts
}

export interface FileView {
  id: string;
  post_id: string;
  original_name?: string;
  mime?: string;
  size_bytes?: number;
  checksum?: string;
  blob_id?: string;
  ticket?: string;
  path: string;
  download_url: string;
  present: boolean;
}

export interface CreateThreadInput {
  title: string;
  body?: string;
  created_at?: string; // Optional override
}

export interface CreatePostInput {
  thread_id?: string; // Usually set by backend from path
  body: string;
  parent_post_ids: string[];
  created_at?: string; // Optional override
}

export interface IdentityInfo {
  gpg_fingerprint: string;
  iroh_peer_id: string;
  friendcode: string;
}

export interface NetworkInfo {
  peer_id: string;
  addresses: string[];
}

export interface HealthResponse {
  status: string;
  version: string;
  api_port: number;
  identity: IdentityInfo;
  network: NetworkInfo;
}

export interface PeerView {
  id: string;
  alias?: string;
  friendcode?: string;
  iroh_peer_id?: string;
  gpg_fingerprint?: string;
  last_seen?: string;
  trust_state: string;
}

export interface AddPeerRequest {
  friendcode: string;
}

export interface ImportResponse {
  id: string;
}
