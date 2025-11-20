import React from 'react';
import { FileText, Reply } from 'lucide-react';
import { api } from '../api/client';
import { type PostView } from '../api/types';
import { formatDistanceToNow } from 'date-fns';
import './Post.css';

interface PostProps {
    post: PostView;
    onReply?: (postId: string) => void;
}

export const Post: React.FC<PostProps> = ({ post, onReply }) => {
    const hasFiles = post.files && post.files.length > 0;

    return (
        <div className="post-container" id={`post-${post.id}`}>
            <div className="post-header">
                <span className="post-author">
                    {post.author_peer_id ? post.author_peer_id.substring(0, 8) : 'Anonymous'}
                </span>
                <span className="post-date">
                    {formatDistanceToNow(new Date(post.created_at))} ago
                </span>
                <span className="post-id" title={post.id}>
                    No. {post.id.substring(0, 8)}
                </span>
                <div className="post-actions">
                    {onReply && (
                        <button className="action-btn" onClick={() => onReply(post.id)}>
                            <Reply size={14} />
                        </button>
                    )}
                </div>
            </div>

            <div className="post-content">
                {hasFiles && (
                    <div className="post-files">
                        {post.files.map((file) => {
                            const isImage = file.mime?.startsWith('image/');
                            const isVideo = file.mime?.startsWith('video/');
                            const url = api.getFileUrl(file.id);

                            return (
                                <div key={file.id} className="file-attachment">
                                    <div className="file-info">
                                        <a href={url} target="_blank" rel="noopener noreferrer" className="file-link">
                                            {file.original_name || file.id.substring(0, 8)}
                                        </a>
                                        <span className="file-meta">
                                            ({file.mime || 'unknown'}, {file.size_bytes ? Math.round(file.size_bytes / 1024) + 'KB' : '?'})
                                        </span>
                                    </div>

                                    {isImage && (
                                        <div className="file-preview">
                                            <img src={url} alt={file.original_name || 'attachment'} loading="lazy" />
                                        </div>
                                    )}

                                    {isVideo && (
                                        <div className="file-preview">
                                            <video src={url} controls loop muted />
                                        </div>
                                    )}

                                    {!isImage && !isVideo && (
                                        <div className="file-generic">
                                            <FileText size={48} />
                                        </div>
                                    )}
                                </div>
                            );
                        })}
                    </div>
                )}

                <div className="post-body">
                    {post.body.split('\n').map((line, i) => (
                        <div key={i} className={line.startsWith('>') ? 'quote-line' : ''}>
                            {line}
                        </div>
                    ))}
                </div>
            </div>
        </div>
    );
};
