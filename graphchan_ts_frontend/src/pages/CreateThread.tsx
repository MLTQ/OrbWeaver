import React, { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { ArrowLeft, Upload } from 'lucide-react';
import { api } from '../api/client';
import { type CreateThreadInput } from '../api/types';
import './CreateThread.css';

export const CreateThread: React.FC = () => {
    const navigate = useNavigate();
    const [title, setTitle] = useState('');
    const [body, setBody] = useState('');
    const [selectedFile, setSelectedFile] = useState<File | null>(null);
    const [loading, setLoading] = useState(false);

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault();
        if (!title.trim() || !body.trim()) return;

        setLoading(true);
        try {
            const input: CreateThreadInput = {
                title,
                body,
            };

            const threadDetails = await api.createThread(input);

            // If there's a file, upload it to the OP post
            if (selectedFile && threadDetails.posts.length > 0) {
                const opPostId = threadDetails.posts[0].id;
                await api.uploadFile(opPostId, selectedFile);
            }

            navigate(`/threads/${threadDetails.thread.id}`);
        } catch (e) {
            console.error('Failed to create thread', e);
            alert('Failed to create thread');
        } finally {
            setLoading(false);
        }
    };

    return (
        <div className="create-thread-container">
            <div className="create-header">
                <button onClick={() => navigate(-1)} className="back-btn">
                    <ArrowLeft size={18} />
                    <span>CANCEL</span>
                </button>
                <h2>INITIATE NEW THREAD</h2>
            </div>

            <form onSubmit={handleSubmit} className="create-form">
                <div className="form-group">
                    <label>SUBJECT</label>
                    <input
                        type="text"
                        value={title}
                        onChange={(e) => setTitle(e.target.value)}
                        placeholder="Enter thread subject..."
                        required
                        className="form-input"
                    />
                </div>

                <div className="form-group">
                    <label>MESSAGE</label>
                    <textarea
                        value={body}
                        onChange={(e) => setBody(e.target.value)}
                        placeholder="Enter your message..."
                        required
                        rows={8}
                        className="form-textarea"
                    />
                </div>

                <div className="form-group">
                    <label>ATTACHMENT (OPTIONAL)</label>
                    <div className="file-upload-area">
                        <input
                            type="file"
                            id="file-upload"
                            onChange={(e) => setSelectedFile(e.target.files?.[0] || null)}
                            className="file-input-hidden"
                        />
                        <label htmlFor="file-upload" className="file-upload-label">
                            <Upload size={20} />
                            <span>{selectedFile ? selectedFile.name : 'SELECT FILE'}</span>
                        </label>
                    </div>
                </div>

                <div className="form-actions">
                    <button type="submit" className="btn btn-large btn-submit" disabled={loading}>
                        {loading ? 'TRANSMITTING...' : 'CREATE THREAD'}
                    </button>
                </div>
            </form>
        </div>
    );
};
