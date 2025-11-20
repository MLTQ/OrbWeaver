import React, { useState, useMemo } from 'react';
import { type PostView } from '../api/types';
import { format } from 'date-fns';
import { ZoomIn, ZoomOut } from 'lucide-react';
import './ThreadTimeline.css';

interface ThreadTimelineProps {
    posts: PostView[];
    onPostClick?: (postId: string) => void;
}

export const ThreadTimeline: React.FC<ThreadTimelineProps> = ({ posts, onPostClick }) => {
    const [zoom, setZoom] = useState(10); // Pixels per minute, default

    const sortedPosts = useMemo(() => {
        return [...posts].sort((a, b) =>
            new Date(a.created_at || 0).getTime() - new Date(b.created_at || 0).getTime()
        );
    }, [posts]);

    const { startTime, durationMinutes } = useMemo(() => {
        if (sortedPosts.length === 0) return { startTime: 0, endTime: 0, durationMinutes: 0 };
        const start = new Date(sortedPosts[0].created_at || 0).getTime();
        const end = new Date(sortedPosts[sortedPosts.length - 1].created_at || 0).getTime();
        return {
            startTime: start,
            endTime: end,
            durationMinutes: (end - start) / 1000 / 60
        };
    }, [sortedPosts]);

    const getPosition = (dateStr: string | undefined) => {
        if (!dateStr) return 0;
        const time = new Date(dateStr).getTime();
        const diffMinutes = (time - startTime) / 1000 / 60;
        return diffMinutes * zoom;
    };

    const handleZoomIn = () => setZoom(prev => Math.min(prev * 1.5, 200));
    const handleZoomOut = () => setZoom(prev => Math.max(prev / 1.5, 1));

    const totalHeight = durationMinutes * zoom + 200; // Extra padding

    return (
        <div className="timeline-container">
            <div className="timeline-controls">
                <button onClick={handleZoomOut} className="zoom-btn" title="Zoom Out">
                    <ZoomOut size={20} />
                </button>
                <span className="zoom-level">{(zoom / 10).toFixed(1)}x</span>
                <button onClick={handleZoomIn} className="zoom-btn" title="Zoom In">
                    <ZoomIn size={20} />
                </button>
            </div>

            <div className="timeline-scroll-area">
                <div className="timeline-track" style={{ height: `${totalHeight}px` }}>
                    <div className="timeline-line" />

                    {sortedPosts.map((post) => {
                        const top = getPosition(post.created_at);
                        return (
                            <div
                                key={post.id}
                                className="timeline-node"
                                style={{ top: `${top}px` }}
                                onClick={() => onPostClick?.(post.id)}
                            >
                                <div className="node-dot" />
                                <div className="node-content">
                                    <div className="node-time">
                                        {post.created_at ? format(new Date(post.created_at), 'HH:mm:ss') : 'Unknown'}
                                    </div>
                                    <div className="node-preview">
                                        <span className="node-id">{`>>${post.id.substring(0, 8)}`}</span>
                                        <span className="node-text">
                                            {post.body.substring(0, 50)}
                                            {post.body.length > 50 ? '...' : ''}
                                        </span>
                                    </div>
                                </div>
                            </div>
                        );
                    })}
                </div>
            </div>
        </div>
    );
};
