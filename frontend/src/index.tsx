// Global CSS
import '../styles/theme.scss';

import * as React from 'react';
import * as ReactDOM from 'react-dom';
import axios from 'axios';
import * as moment from 'moment';
import * as qs from 'qs';

type Answer = {
  id: string,
  title: string,
  title_html: string,
  content: string,
  content_html: string,
  answered: boolean,
  asof: string,
};

class AnswerEdit extends React.Component {
  props: {
    id?: string,
    title: string,
    content: string,
    onClick: any,
    focus?: string,
  };

  state = {
    title: this.props.title,
    content: this.props.content,
  };

  render(): React.ReactNode {
    return (
      <div
        className={`answer editing answered`}
      >
        <input
          autoFocus={this.props.focus === 'title'}
          className="title"
          type="text"
          value={this.state.title}
          onChange={(e) => {
            this.setState({
              title: e.target.value,
            });
          }}
        />
        <div className="as-of">
          <span>As of {moment().format("MMMM YYYY")}.</span>
        </div>
        <textarea
          autoFocus={this.props.focus === 'content'}
          className="content"
          value={this.state.content}
          onChange={(e) => {
            this.setState({
              content: e.target.value,
            });
          }}
        />
        <div className="controls">
          <button
            className="success"
            onClick={(e) => {
              (typeof this.props.id !== 'string' ?
                axios.post('/api/new', qs.stringify({
                  title: this.state.title,
                  content: this.state.content,
                })) :
                axios.post('/api/edit', qs.stringify({
                  title: this.state.title,
                  content: this.state.content,
                  id: this.props.id,
                }))
              )
              .then((e) => {
                location.reload();
              })
              .catch(err => {
                console.error(err);
                alert('error, check console.');
              })
            }}
          >
            Save
          </button>
          <button
            onClick={this.props.onClick}
          >
            Cancel
          </button>
        </div>
      </div>
    );
  }
}

class AnswerView extends React.Component {
  props: {
    item: Answer,
    loggedIn: boolean,
  };

  state = {
    editing: false,
    collapsed: !this.props.item.answered,
  };

  render(): React.ReactNode {
    let item = this.props.item;
    if (this.state.editing) {
      return (
        <AnswerEdit
          id={this.props.item.id}
          title={this.props.item.title}
          content={this.props.item.content}
          focus="content"
          onClick={(e) => {
            this.setState({
              editing: false,
            });
          }}
        />
      );
    } else {
      return (
        <div
          id={`a${item.id}`}
          className={`
            answer
            ${item.answered ? 'answered' : ''}
            ${this.state.collapsed ? 'collapsed' : ''}
          `}
          data-content={(item.title + '\n\n' + item.content).toLowerCase()}
        >
          <div
            className="title"
            dangerouslySetInnerHTML={{
              __html: item.title_html.replace(/<\/h2>/, ` <a class="answer-anchor" href="#a${item.id}">#</a>`),
            }}
            style={{
              cursor: 'pointer',
            }}
            onClick={(e) => {
              // Don't target the answer-anchor
              if ((e.target as any).tagName.toLowerCase() != 'a') {
                this.setState({
                  collapsed: !this.state.collapsed,
                })
                e.preventDefault();
              }
            }}
          />
          <div className="as-of">
            <span>As of {item.asof}.</span>
            {this.props.loggedIn ?
              <a className="edit" onClick={(e) => {
                if (this.props.loggedIn) {
                  // Switch to editor mode
                  this.setState({
                    editing: true,
                  });
                }
              }}>️✏️ Edit</a>
              : null}
          </div>

          <div
            className="content"
            dangerouslySetInnerHTML={{__html: item.content_html}}
          />
        </div>
      );
    }
  }
}

class Answers extends React.Component {
  props: {
    answers: Array<Answer>,
    loggedIn: boolean,
  };

  style: any;

  state = {
    loggedIn: this.props.loggedIn,
    answers: this.props.answers,
    newItem: false,
  };

  constructor(props) {
    super(props);

    var style = document.createElement('style');
    style.type = 'text/css';
    style.innerHTML = '';
    document.getElementsByTagName('head')[0].appendChild(style);
    this.style = style;
  }

  render(): React.ReactNode {
    const self = this;

    const answers = this.state.answers.map((item, i) => {
      return (
        <AnswerView
          key={i}
          loggedIn={self.state.loggedIn}
          item={item}
        />
      );
    });

    if (this.state.newItem) {
      answers.unshift(
        <AnswerEdit
          key="@editing"
          title="New Question"
          content=""
          focus="title"
          onClick={(e) => {
            this.setState({
              newItem: false,
            });
          }}
        />
      )
    }

    return (
      <div id="content-render">
        <div id="header">
          <h1>tim answered this</h1>
          {self.state.loggedIn ?
            <div className="caption">
              <a href="#" onClick={(e) => this.setState({newItem: true})}>Submit new answer?</a>
            </div>
            : null
          }
        </div>
        <div id="search">
          <span>{'Search: '}</span>
          <input
            type="text"
            onChange={(e) => {
              let filters = e.target.value.toLowerCase().replace(/^\s*|\s*$/g, '').split(/\s+/)
                .filter(x => x.length > 0);
              let selectors = filters.map(x => {
                return `[data-content*=${JSON.stringify(x)}]`
              }).join('');
              console.log(selectors);
              this.style.innerText = filters.length == 0 ? ''
                : `.answer { display: none; } .answer${selectors} { display: block; }`
            }}
          />
        </div>
        {answers}
        <div id="footer">
          &copy; 2018 AnsweredThis.com
        </div>
      </div>
    );
  }
}

function start() {
  // Create the editor frame.
  axios.get('/api/answers/')
  .then((res) => {
    ReactDOM.render(
      <Answers
        answers={res.data.answers}
        loggedIn={res.data.logged_in}
      />,
      document.querySelector('#content')!,
    );
  })
  .catch((err) => {
    console.log(err);
  });
}

document.addEventListener('DOMContentLoaded', start);
