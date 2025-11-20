import React, { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { Activity, Clock, Hash, MessageSquare, Users } from 'lucide-react';
import { api } from '../api/client';
import { type HealthResponse, type ThreadSummary } from '../api/types';
import { formatDistanceToNow } from 'date-fns';
import './Dashboard.css';

export const Dashboard: React.FC = () => {
    const [health, setHealth] = useState<HealthResponse | null>(null);
    const [recentThreads, setRecentThreads] = useState<ThreadSummary[]>([]);
    const [loading, setLoading] = useState(true);

    useEffect(() => {
        const fetchData = async () => {
            try {
                const [healthData, threadsData] = await Promise.all([
                    api.getHealth(),
                    api.listThreads(5),
                ]);
                setHealth(healthData);
                setRecentThreads(threadsData);
            } catch (e) {
                console.error('Failed to fetch dashboard data', e);
            } finally {
                setLoading(false);
            }
        };
        fetchData();
    }, []);

    if (loading) {
        return <div className="loading-screen">INITIALIZING SYSTEM...</div>;
    }

    return (
        <div className="dashboard-container">
            {/* Top Stats Row */}
            <div className="stats-grid">
                <div className="stat-card">
                    <div className="stat-header">
                        <Activity size={20} />
                        <span>SYSTEM STATUS</span>
                    </div>
                    <div className="stat-value text-success">
                        {health?.status === 'ok' ? 'OPERATIONAL' : 'ERROR'}
                    </div>
                    <div className="stat-sub">
                        UPTIME: {Math.floor(performance.now() / 1000)}s
                    </div>
                </div>

                <div className="stat-card">
                    <div className="stat-header">
                        <Users size={20} />
                        <span>NETWORK PEERS</span>
                    </div>
                    <div className="stat-value text-cyan">
                        {health?.network.addresses.length || 0}
                    </div>
                    <div className="stat-sub">
                        ID: {health?.network.peer_id.substring(0, 8)}...
                    </div>
                </div>

                <div className="stat-card">
                    <div className="stat-header">
                        <Hash size={20} />
                        <span>ACTIVE THREADS</span>
                    </div>
                    <div className="stat-value text-accent">
                        {recentThreads.length}
                    </div>
                    <div className="stat-sub">
                        LAST SYNC: NOW
                    </div>
                </div>
            </div>

            {/* Recent Activity Section */}
            <div className="section-container">
                <div className="section-header">
                    <Clock size={18} />
                    <h2>RECENT TRANSMISSIONS</h2>
                </div>

                <div className="thread-list">
                    {recentThreads.map((thread) => (
                        <Link to={`/threads/${thread.id}`} key={thread.id} className="thread-item">
                            <div className="thread-info">
                                <h3 className="thread-title">{thread.title}</h3>
                                <div className="thread-meta">
                                    <span>ID: {thread.id.substring(0, 8)}</span>
                                    <span>•</span>
                                    <span>{formatDistanceToNow(new Date(thread.created_at))} ago</span>
                                </div>
                            </div>
                            <div className="thread-stats">
                                <MessageSquare size={14} />
                                <span>{thread.post_count}</span>
                            </div>
                        </Link>
                    ))}

                    {recentThreads.length === 0 && (
                        <div className="empty-state">NO TRANSMISSIONS DETECTED</div>
                    )}
                </div>
            </div>

            <div className="quick-actions">
                <Link to="/threads/new" className="btn btn-large">
                    INITIATE NEW THREAD
                </Link>
                <Link to="/peers" className="btn btn-cyan btn-large">
                    MANAGE NETWORK
                </Link>
            </div>
        </div>
    );
};
