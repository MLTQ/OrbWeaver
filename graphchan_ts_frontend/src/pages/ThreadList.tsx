import React, { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { Hash, MessageSquare, Plus, Search } from 'lucide-react';
import { api } from '../api/client';
import { type ThreadSummary } from '../api/types';
import { formatDistanceToNow } from 'date-fns';
import './ThreadList.css';

export const ThreadList: React.FC = () => {
    const [threads, setThreads] = useState<ThreadSummary[]>([]);
    const [loading, setLoading] = useState(true);
    const [searchTerm, setSearchTerm] = useState('');

    useEffect(() => {
        const fetchThreads = async () => {
            try {
                const data = await api.listThreads(100);
                setThreads(data);
            } catch (e) {
                console.error('Failed to fetch threads', e);
            } finally {
                setLoading(false);
            }
        };
        fetchThreads();
    }, []);

    const filteredThreads = threads.filter((t) =>
        t.title.toLowerCase().includes(searchTerm.toLowerCase()) ||
        t.id.includes(searchTerm)
    );

    if (loading) {
        return <div className="loading-screen">ACCESSING ARCHIVES...</div>;
    }

    return (
        <div className="thread-list-container">
            <div className="list-header">
                <div className="search-bar">
                    <Search size={18} className="search-icon" />
                    <input
                        type="text"
                        placeholder="SEARCH THREADS..."
                        value={searchTerm}
                        onChange={(e) => setSearchTerm(e.target.value)}
                        className="search-input"
                    />
                </div>
                <Link to="/threads/new" className="btn btn-large">
                    <Plus size={18} />
                    <span>NEW THREAD</span>
                </Link>
            </div>

            <div className="threads-grid">
                {filteredThreads.map((thread) => (
                    <Link to={`/threads/${thread.id}`} key={thread.id} className="thread-card">
                        <div className="card-top">
                            <Hash size={16} className="text-accent" />
                            <span className="thread-id">{thread.id.substring(0, 8)}</span>
                            {thread.pinned && <span className="badge-pinned">PINNED</span>}
                        </div>

                        <h3 className="card-title">{thread.title}</h3>

                        <div className="card-meta">
                            <div className="meta-item">
                                <MessageSquare size={14} />
                                <span>{thread.post_count} POSTS</span>
                            </div>
                            <div className="meta-item">
                                <span>LAST ACT: {formatDistanceToNow(new Date(thread.last_bump_at))} AGO</span>
                            </div>
                        </div>
                    </Link>
                ))}
            </div>

            {filteredThreads.length === 0 && (
                <div className="empty-state">NO RECORDS FOUND</div>
            )}
        </div>
    );
};
