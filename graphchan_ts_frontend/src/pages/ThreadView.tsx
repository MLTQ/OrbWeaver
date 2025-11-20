import React, { useEffect, useState } from 'react';
import { useParams, Link } from 'react-router-dom';
import { ArrowLeft, Send, List, Activity, Clock } from 'lucide-react';
import { api } from '../api/client';
import { type ThreadDetails, type CreatePostInput } from '../api/types';
import { Post } from '../components/Post';
import { ThreadGraph } from '../components/ThreadGraph';
import './ThreadView.css';

type ViewMode = 'LIST' | 'GRAPH' | 'TIMELINE';

export const ThreadView: React.FC = () => {
  const { id } = useParams<{ id: string }>();
  const [thread, setThread] = useState<ThreadDetails | null>(null);
  const [loading, setLoading] = useState(true);
  const [replyBody, setReplyBody] = useState('');
  const [sending, setSending] = useState(false);
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const [viewMode, setViewMode] = useState<ViewMode>('LIST');

  const fetchThread = async () => {
    if (!id) return;
    try {
      const data = await api.getThread(id);
      setThread(data);
    } catch (e) {
      console.error('Failed to fetch thread', e);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchThread();
  }, [id]);

  const handleReply = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!id || !replyBody.trim()) return;

    setSending(true);
    try {
      const input: CreatePostInput = {
        body: replyBody,
        parent_post_ids: [],
      };

      if (thread && thread.posts.length > 0) {
        input.parent_post_ids.push(thread.posts[0].id);
      }

      const newPost = await api.createPost(id, input);

      if (selectedFile) {
        await api.uploadFile(newPost.id, selectedFile);
      }

      setReplyBody('');
      setSelectedFile(null);
      fetchThread();
    } catch (e) {
      console.error('Failed to post reply', e);
      alert('Failed to post reply');
    } finally {
      setSending(false);
    }
  };

  if (loading) {
    return <div className="loading-screen">DECRYPTING THREAD DATA...</div>;
  }

  if (!thread) {
    return <div className="error-screen">THREAD NOT FOUND OR DELETED</div>;
  }

  return (
    <div className="thread-view-container">
      <div className="view-header">
        <Link to="/threads" className="back-link">
          <ArrowLeft size={18} />
          <span>RETURN TO INDEX</span>
        </Link>
        <h2 className="thread-view-title">/{thread.thread.title}/</h2>

        <div className="view-controls">
          <button
            className={`view-btn ${viewMode === 'LIST' ? 'active' : ''}`}
            onClick={() => setViewMode('LIST')}
            title="List View"
          >
            <List size={18} />
          </button>
          <button
            className={`view-btn ${viewMode === 'GRAPH' ? 'active' : ''}`}
            onClick={() => setViewMode('GRAPH')}
            title="Graph View"
          >
            <Activity size={18} />
          </button>
          <button
            className={`view-btn ${viewMode === 'TIMELINE' ? 'active' : ''}`}
            onClick={() => setViewMode('TIMELINE')}
            title="Timeline View"
          >
            <Clock size={18} />
          </button>
        </div>

        <div className="thread-view-meta">
          {thread.thread.post_count} POSTS
        </div>
      </div>

      {viewMode === 'LIST' && (
        <div className="posts-list">
          {thread.posts.map((post) => (
            <Post
              key={post.id}
              post={post}
              onReply={(postId) => setReplyBody((prev) => prev + `>>${postId}\n`)}
            />
          ))}
        </div>
      )}

      {viewMode === 'GRAPH' && (
        <div className="graph-container">
          <ThreadGraph
            posts={thread.posts}
            onNodeClick={(postId) => {
              console.log('Clicked node:', postId);
            }}
          />
        </div>
      )}

      {viewMode === 'TIMELINE' && (
        <div className="timeline-placeholder p-8 text-center border border-[var(--border-color)] bg-[var(--bg-secondary)]">
          TIMELINE PROJECTION NOT YET CALIBRATED
        </div>
      )}

      <div className="reply-form-container">
        <form onSubmit={handleReply} className="reply-form">
          <div className="form-header">POST A REPLY</div>
          <textarea
            value={replyBody}
            onChange={(e) => setReplyBody(e.target.value)}
            placeholder="Enter your message..."
            className="reply-input"
            rows={4}
          />
          <div className="form-actions">
            <input
              type="file"
              onChange={(e) => setSelectedFile(e.target.files?.[0] || null)}
              className="file-input"
            />
            <button type="submit" className="btn btn-large" disabled={sending}>
              {sending ? 'TRANSMITTING...' : 'POST REPLY'}
              {!sending && <Send size={16} className="ml-2" />}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
