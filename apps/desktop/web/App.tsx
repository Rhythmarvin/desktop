import React, { useState, useEffect } from 'react';
import * as ScrollArea from '@radix-ui/react-scroll-area';
import * as Avatar from '@radix-ui/react-avatar';
import { Send, Plus, MessageSquare } from 'lucide-react';

interface ChatMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
}

interface ChatSession {
  id: string;
  title: string;
  date: string;
}

function App() {
  const [chats, setChats] = useState<ChatSession[]>([]);
  const [activeSession, setActiveSession] = useState<string | null>(null);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [inputValue, setInputValue] = useState('');

  useEffect(() => {
    fetch('/api/chats')
      .then(res => res.json())
      .then(res => {
        if (res.code === 0) {
          setChats(res.data);
          if (res.data.length > 0) {
            handleSelectSession(res.data[0].id);
          }
        }
      });
  }, []);

  const handleSelectSession = (id: string) => {
    setActiveSession(id);
    fetch(`/api/chats/${id}`)
      .then(res => res.json())
      .then(res => {
        if (res.code === 0 && res.data) {
          setMessages(res.data.messages || []);
        } else {
          setMessages([]);
        }
      })
      .catch(() => setMessages([]));
  };

  const startNewChat = () => {
    const newId = Date.now().toString();
    const newChat = { id: newId, title: 'New Conversation', date: new Date().toISOString().split('T')[0] };
    setChats([newChat, ...chats]);
    setActiveSession(newId);
    setMessages([]);
  };

  const handleSend = () => {
    if (!inputValue.trim()) return;
    const newMessage: ChatMessage = { id: Date.now().toString(), role: 'user', content: inputValue };
    setMessages([...messages, newMessage]);
    setInputValue('');
    
    setTimeout(() => {
      setMessages(prev => [...prev, { id: Date.now().toString(), role: 'assistant', content: 'This is a mocked response.' }]);
    }, 600);
  };

  return (
    <div className="layout">
      <div className="sidebar" style={{ padding: '8px' }}>
        <button className="button button-primary" style={{ width: '100%', marginBottom: '16px' }} onClick={startNewChat}>
          <Plus size={16} style={{ marginRight: '8px' }} />
          New Chat
        </button>

        <div style={{ flex: 1, overflow: 'hidden' }}>
          <ScrollArea.Root className="ScrollAreaRoot" style={{ width: '100%', height: '100%' }}>
            <ScrollArea.Viewport className="ScrollAreaViewport" style={{ width: '100%', height: '100%' }}>
              <div style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                {chats.map(chat => (
                  <div
                    key={chat.id}
                    onClick={() => handleSelectSession(chat.id)}
                    style={{
                      padding: '8px 12px',
                      borderRadius: 'var(--radius-sm)',
                      cursor: 'pointer',
                      display: 'flex',
                      alignItems: 'center',
                      gap: '8px',
                      backgroundColor: activeSession === chat.id ? 'var(--border-subtle)' : 'transparent',
                      color: activeSession === chat.id ? 'var(--fg)' : 'var(--fg-secondary)',
                      fontSize: '13px',
                    }}
                  >
                    <MessageSquare size={14} />
                    <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                      {chat.title}
                    </span>
                  </div>
                ))}
              </div>
            </ScrollArea.Viewport>
            <ScrollArea.Scrollbar className="ScrollAreaScrollbar" orientation="vertical">
              <ScrollArea.Thumb className="ScrollAreaThumb" />
            </ScrollArea.Scrollbar>
          </ScrollArea.Root>
        </div>
      </div>

      <div className="main-content">
        <div style={{ padding: '12px 24px', borderBottom: '1px solid var(--border)', fontWeight: 500 }}>
          {activeSession ? chats.find(c => c.id === activeSession)?.title : 'Select a conversation'}
        </div>
        
        <div style={{ flex: 1, overflow: 'hidden' }}>
          <ScrollArea.Root className="ScrollAreaRoot" style={{ width: '100%', height: '100%' }}>
            <ScrollArea.Viewport className="ScrollAreaViewport" style={{ width: '100%', height: '100%' }}>
              <div style={{ padding: '24px', display: 'flex', flexDirection: 'column', gap: '24px', maxWidth: '800px', margin: '0 auto' }}>
                {messages.map(msg => (
                  <div key={msg.id} style={{ display: 'flex', gap: '16px' }}>
                    <Avatar.Root className="AvatarRoot" style={{ flexShrink: 0 }}>
                      <Avatar.Fallback className="AvatarFallback" style={{
                        backgroundColor: msg.role === 'assistant' ? 'var(--primary)' : 'var(--bg-secondary)',
                        color: msg.role === 'assistant' ? '#fff' : 'var(--fg)',
                        width: '32px', height: '32px', display: 'flex', alignItems: 'center', justifyContent: 'center',
                        borderRadius: 'var(--radius-md)', fontSize: '12px', fontWeight: 600
                      }}>
                        {msg.role === 'assistant' ? 'AI' : 'U'}
                      </Avatar.Fallback>
                    </Avatar.Root>
                    <div style={{ flex: 1, paddingTop: '4px', lineHeight: 1.5 }}>
                      <div style={{ fontWeight: 600, fontSize: '13px', marginBottom: '4px', color: 'var(--fg-secondary)' }}>
                        {msg.role === 'assistant' ? 'Assistant' : 'You'}
                      </div>
                      <div style={{ fontSize: '14px', color: 'var(--fg)' }}>
                        {msg.content}
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            </ScrollArea.Viewport>
            <ScrollArea.Scrollbar className="ScrollAreaScrollbar" orientation="vertical">
              <ScrollArea.Thumb className="ScrollAreaThumb" />
            </ScrollArea.Scrollbar>
          </ScrollArea.Root>
        </div>

        <div style={{ padding: '24px', display: 'flex', justifyContent: 'center', borderTop: '1px solid var(--border)' }}>
          <div style={{ display: 'flex', gap: '8px', maxWidth: '800px', width: '100%' }}>
            <input 
              type="text" 
              className="input" 
              style={{ flex: 1 }} 
              placeholder="Ask me anything..." 
              value={inputValue}
              onChange={e => setInputValue(e.target.value)}
              onKeyDown={e => { if (e.key === 'Enter') handleSend(); }}
            />
            <button className="button button-primary" onClick={handleSend} style={{ padding: '8px' }}>
              <Send size={16} />
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

export default App;
